use anyhow::Result;
use object::write::{Object, StandardSection, Symbol, SymbolSection};
use object::{Architecture, BinaryFormat, Endianness, SymbolFlags, SymbolKind, SymbolScope};

use crate::ir::{self, Terminator, Value};

#[derive(Debug, Clone, Copy)]
enum MInst {
    MovImm { dst: Reg, imm: i32 },
    Ret,
}

#[derive(Debug, Clone, Copy)]
enum Reg {
    Eax,
}

impl Reg {
    fn code(self) -> u8 {
        match self {
            Reg::Eax => 0,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Reg::Eax => "eax",
        }
    }
}

fn select(func: &ir::Function) -> Vec<MInst> {
    let mut out = Vec::new();
    for block in &func.blocks {
        // `block.insts` is empty until value-producing instructions exist, so
        // only terminators lower to anything for now. Multiple blocks would
        // need labels/branches — a single-block concern to solve with control
        // flow.
        match &block.term {
            Terminator::Ret(Some(Value::Const(c))) => {
                out.push(MInst::MovImm { dst: Reg::Eax, imm: *c as i32 });
                out.push(MInst::Ret);
            }
            Terminator::Ret(None) => out.push(MInst::Ret),
        }
    }
    out
}

fn encode(inst: MInst, out: &mut Vec<u8>) {
    match inst {
        // B8+rd id : mov r32, imm32
        MInst::MovImm { dst, imm } => {
            out.push(0xB8 + dst.code());
            out.extend_from_slice(&imm.to_le_bytes());
        }
        // C3 : ret
        MInst::Ret => out.push(0xC3),
    }
}

fn asm(inst: MInst) -> String {
    match inst {
        MInst::MovImm { dst, imm } => format!("mov {}, {imm}", dst.name()),
        MInst::Ret => "ret".to_string(),
    }
}

pub fn assembly(program: &ir::Program) -> String {
    let mut s = String::new();
    for func in &program.functions {
        s.push_str(&format!("{}:\n", func.name));
        for inst in select(func) {
            s.push_str(&format!("    {}\n", asm(inst)));
        }
    }
    s
}

pub fn emit_object(program: &ir::Program) -> Result<Vec<u8>> {
    let mut obj = Object::new(BinaryFormat::Elf, Architecture::X86_64, Endianness::Little);
    let text = obj.section_id(StandardSection::Text);

    for func in &program.functions {
        let mut code = Vec::new();
        for inst in select(func) {
            encode(inst, &mut code);
        }
        let offset = obj.append_section_data(text, &code, 16);

        obj.add_symbol(Symbol {
            name: func.name.clone().into_bytes(),
            value: offset,
            size: code.len() as u64,
            kind: SymbolKind::Text,
            scope: SymbolScope::Linkage, // global, so the linker can resolve `main`
            weak: false,
            section: SymbolSection::Section(text),
            flags: SymbolFlags::None,
        });
    }

    obj.write()
        .map_err(|e| anyhow::anyhow!("object write failed: {e}"))
}
