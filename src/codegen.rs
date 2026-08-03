use anyhow::anyhow;
use object::write::{Object, StandardSection, Symbol, SymbolSection};
use object::{Architecture, BinaryFormat, Endianness, SymbolFlags, SymbolKind, SymbolScope};

use crate::diagnostic::Error;
use crate::ir::{self, IBinOp, IPred, InstKind, SlotId, Terminator, Value};

type Result<T> = std::result::Result<T, Error>;

// ----- registers & frame layout (BE-1) -------------------------------------

/// The general-purpose registers this backend touches: the scratch/frame set
/// plus the SysV integer-argument registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gpr {
    Rax,
    Rcx,
    Rdx,
    Rsp,
    Rbp,
    Rsi,
    Rdi,
    R8,
    R9,
}

impl Gpr {
    /// The register number used in ModRM/opcode encodings; values >= 8 need a
    /// REX extension bit, which the encoder adds.
    fn code(self) -> u8 {
        match self {
            Gpr::Rax => 0,
            Gpr::Rcx => 1,
            Gpr::Rdx => 2,
            Gpr::Rsp => 4,
            Gpr::Rbp => 5,
            Gpr::Rsi => 6,
            Gpr::Rdi => 7,
            Gpr::R8 => 8,
            Gpr::R9 => 9,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Gpr::Rax => "rax",
            Gpr::Rcx => "rcx",
            Gpr::Rdx => "rdx",
            Gpr::Rsp => "rsp",
            Gpr::Rbp => "rbp",
            Gpr::Rsi => "rsi",
            Gpr::Rdi => "rdi",
            Gpr::R8 => "r8",
            Gpr::R9 => "r9",
        }
    }

    /// The low-byte register name, for `setcc`/`movzx` textual output.
    fn name8(self) -> &'static str {
        match self {
            Gpr::Rax => "al",
            Gpr::Rcx => "cl",
            Gpr::Rdx => "dl",
            Gpr::Rsp => "spl",
            Gpr::Rbp => "bpl",
            Gpr::Rsi => "sil",
            Gpr::Rdi => "dil",
            Gpr::R8 => "r8b",
            Gpr::R9 => "r9b",
        }
    }

    /// The 32-bit register name, for `movsxd` textual output.
    fn name32(self) -> &'static str {
        match self {
            Gpr::Rax => "eax",
            Gpr::Rcx => "ecx",
            Gpr::Rdx => "edx",
            Gpr::Rsp => "esp",
            Gpr::Rbp => "ebp",
            Gpr::Rsi => "esi",
            Gpr::Rdi => "edi",
            Gpr::R8 => "r8d",
            Gpr::R9 => "r9d",
        }
    }
}

/// The SysV integer-argument registers, in order.
const ARG_REGS: [Gpr; 6] = [Gpr::Rdi, Gpr::Rsi, Gpr::Rdx, Gpr::Rcx, Gpr::R8, Gpr::R9];

/// The byte displacement of SSA value `%v`'s spill slot: `[rbp - 8*(v+1)]`
/// (§2.1 spill-everything: one fixed 8-byte slot per value).
fn spill(v: u32) -> i32 {
    -8 * (v as i32 + 1)
}

fn align_to(n: i32, a: i32) -> i32 {
    (n + a - 1) / a * a
}

// ----- machine instructions (BE-1 / BE-3) ----------------------------------

/// x86-64 ALU operations sharing the reg/reg `OP r/m64, r64` encoding form.
/// The full BE-3 set is defined; only `Add`/`Sub` are produced by lowering in
/// v1 (the rest are wired as their IR ops land).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum AluOp {
    Add,
    Sub,
    And,
    Or,
    Xor,
    Cmp,
}

impl AluOp {
    /// The primary opcode of the `OP r/m64, r64` form.
    fn opcode(self) -> u8 {
        match self {
            AluOp::Add => 0x01,
            AluOp::Sub => 0x29,
            AluOp::And => 0x21,
            AluOp::Or => 0x09,
            AluOp::Xor => 0x31,
            AluOp::Cmp => 0x39,
        }
    }

    fn name(self) -> &'static str {
        match self {
            AluOp::Add => "add",
            AluOp::Sub => "sub",
            AluOp::And => "and",
            AluOp::Or => "or",
            AluOp::Xor => "xor",
            AluOp::Cmp => "cmp",
        }
    }
}

/// Variable-count shifts (`OP r/m64, cl`), sharing the `D3` opcode and taking
/// their count implicitly in `cl`. `Shr` (logical) is reserved for unsigned
/// types, which the subset does not have yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum ShiftOp {
    Shl,
    Sar,
    Shr,
}

impl ShiftOp {
    /// The ModRM `reg` extension digit selecting the shift within the `D3` group.
    fn ext(self) -> u8 {
        match self {
            ShiftOp::Shl => 4, // /4
            ShiftOp::Shr => 5, // /5
            ShiftOp::Sar => 7, // /7
        }
    }

    fn name(self) -> &'static str {
        match self {
            ShiftOp::Shl => "shl",
            ShiftOp::Shr => "shr",
            ShiftOp::Sar => "sar",
        }
    }
}

/// Condition codes for `setcc` (and later `jcc`). The signed and equality codes
/// are produced in v1; the unsigned orderings (B/Ae/Be/A) are reserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum Cc {
    E,
    Ne,
    L,
    Ge,
    Le,
    G,
    B,
    Ae,
    Be,
    A,
}

impl Cc {
    /// The tttn nibble added to the `0F 90` (setcc) / `0F 80` (jcc) opcode base.
    fn code(self) -> u8 {
        match self {
            Cc::E => 0x4,
            Cc::Ne => 0x5,
            Cc::B => 0x2,
            Cc::Ae => 0x3,
            Cc::Be => 0x6,
            Cc::A => 0x7,
            Cc::L => 0xC,
            Cc::Ge => 0xD,
            Cc::Le => 0xE,
            Cc::G => 0xF,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Cc::E => "e",
            Cc::Ne => "ne",
            Cc::L => "l",
            Cc::Ge => "ge",
            Cc::Le => "le",
            Cc::G => "g",
            Cc::B => "b",
            Cc::Ae => "ae",
            Cc::Be => "be",
            Cc::A => "a",
        }
    }
}

/// The condition code that materializes an integer comparison predicate.
fn cc_of(pred: IPred) -> Cc {
    match pred {
        IPred::Eq => Cc::E,
        IPred::Ne => Cc::Ne,
        IPred::SLt => Cc::L,
        IPred::SLe => Cc::Le,
        IPred::SGt => Cc::G,
        IPred::SGe => Cc::Ge,
        IPred::ULt => Cc::B,
        IPred::ULe => Cc::Be,
        IPred::UGt => Cc::A,
        IPred::UGe => Cc::Ae,
    }
}

#[derive(Debug, Clone, Copy)]
enum MInst {
    PushRbp,
    MovRbpRsp,
    SubRspImm(i32),
    Leave,
    Ret,
    // `mov r64, imm`
    MovRImm { dst: Gpr, imm: i64 },
    // `mov r64, [rbp+disp]` (load a spill slot)
    Load { dst: Gpr, disp: i32 },
    // `mov [rbp+disp], r64` (store a spill slot)
    Store { disp: i32, src: Gpr },
    // `<op> dst, src` — reg/reg ALU.
    Alu { op: AluOp, dst: Gpr, src: Gpr },
    // `imul dst, src` (0F AF)
    IMul { dst: Gpr, src: Gpr },
    // `cqo` — sign-extend rax into rdx:rax ahead of `idiv`.
    Cqo,
    // `idiv src` (F7 /7) — rax = rdx:rax / src, rdx = remainder.
    Idiv { src: Gpr },
    // `<op> dst, cl` (D3 /ext) — shift `dst` by the count in `cl`.
    Shift { op: ShiftOp, dst: Gpr },
    // `setcc dst_l` (0F 90+cc /0) — set `dst`'s low byte to the flag condition.
    SetCC { cc: Cc, dst: Gpr },
    // `movzx dst, dst_l` (0F B6 /r) — zero-extend `dst`'s low byte into `dst`.
    Movzx8 { dst: Gpr },
    // `movsxd dst, src_32` (63 /r) — sign-extend `src`'s low dword into `dst`.
    Movsxd { dst: Gpr, src: Gpr },
    // A basic-block label (zero bytes); records its offset for jump fixups.
    Label(u32),
    // jmp rel32 to a block label.
    Jmp(u32),
    // jcc rel32 to a block label.
    JmpCC { cc: Cc, target: u32 },
    // test r/m64, r64 — sets flags from `a & b` (used as `test r, r`).
    Test { a: Gpr, b: Gpr },
}

// ----- instruction selection (§2.1 spill-everything) -----------------------

struct FuncSel<'a> {
    func: &'a ir::Function,
    /// Number of SSA values; the locals' stack slots sit below their spills.
    n_values: u32,
    /// 16-byte-aligned frame size reserved by the prologue.
    frame: i32,
}

impl<'a> FuncSel<'a> {
    fn new(func: &'a ir::Function) -> Self {
        // One 8-byte slot per SSA value. Values are numbered densely from 0, so
        // the highest `dst` + 1 is the count.
        let n_values = func
            .blocks
            .iter()
            .flat_map(|b| b.insts.iter())
            .filter_map(|i| i.dst())
            .map(|d| d + 1)
            .max()
            .unwrap_or(0);
        // The frame: spill slots first, then one 8-byte slot per local. Every
        // slot is 8 bytes so an `int` local holds its canonical sign-extended
        // form, like any other value.
        let frame = align_to((n_values as i32 + func.slots.len() as i32) * 8, 16);
        Self { func, n_values, frame }
    }

    /// The byte displacement of local stack slot `$s`, below the spill area.
    fn local(&self, s: SlotId) -> i32 {
        -8 * (self.n_values as i32 + s.0 as i32 + 1)
    }

    fn select(&self) -> Result<Vec<MInst>> {
        let mut code = vec![MInst::PushRbp, MInst::MovRbpRsp];
        if self.frame > 0 {
            code.push(MInst::SubRspImm(self.frame));
        }
        // Spill the incoming arguments (SysV: rdi..r9) to their parameter
        // slots, sign-extending from 32 bits: the upper halves of argument
        // registers are unspecified at entry, and the slots must hold the
        // canonical sign-extended form like every other value.
        for (i, _) in self.func.params.iter().enumerate() {
            let reg = ARG_REGS[i];
            code.push(MInst::Movsxd { dst: reg, src: reg });
            code.push(MInst::Store { disp: self.local(SlotId(i as u32)), src: reg });
        }
        for block in &self.func.blocks {
            code.push(MInst::Label(block.id.0));
            for inst in &block.insts {
                self.inst(inst, &mut code)?;
            }
            self.term(&block.term, &mut code);
        }
        Ok(code)
    }

    /// Load an IR operand into `reg`: an immediate is materialized, a register
    /// operand is loaded from its slot.
    fn load(&self, v: &Value, reg: Gpr, code: &mut Vec<MInst>) -> Result<()> {
        match v {
            Value::Const(c) => code.push(MInst::MovRImm { dst: reg, imm: *c }),
            Value::Reg(n) => code.push(MInst::Load { dst: reg, disp: spill(*n) }),
            Value::FConst(_) => {
                return Err(
                    self.err("floating-point operands are not yet supported in codegen (TBD)")
                );
            }
        }
        Ok(())
    }

    fn inst(&self, inst: &ir::Inst, code: &mut Vec<MInst>) -> Result<()> {
        match &inst.kind {
            // Execute: load operands into rax/rcx, compute, store to `dst`.
            InstKind::IBin { dst, op, lhs, rhs, .. } => {
                self.load(lhs, Gpr::Rax, code)?;
                self.load(rhs, Gpr::Rcx, code)?;
                let result = match op {
                    IBinOp::Add => {
                        code.push(MInst::Alu { op: AluOp::Add, dst: Gpr::Rax, src: Gpr::Rcx });
                        Gpr::Rax
                    }
                    IBinOp::Sub => {
                        code.push(MInst::Alu { op: AluOp::Sub, dst: Gpr::Rax, src: Gpr::Rcx });
                        Gpr::Rax
                    }
                    IBinOp::Mul => {
                        code.push(MInst::IMul { dst: Gpr::Rax, src: Gpr::Rcx });
                        Gpr::Rax
                    }
                    IBinOp::SDiv => {
                        code.push(MInst::Cqo);
                        code.push(MInst::Idiv { src: Gpr::Rcx });
                        Gpr::Rax // quotient
                    }
                    IBinOp::SRem => {
                        code.push(MInst::Cqo);
                        code.push(MInst::Idiv { src: Gpr::Rcx });
                        Gpr::Rdx // remainder
                    }
                    // Bitwise ops are bit-parallel, so operating on the full
                    // 64-bit sign-extended operands keeps the result in the
                    // canonical form (its bit 63 tracks the `int` sign bit).
                    IBinOp::And => {
                        code.push(MInst::Alu { op: AluOp::And, dst: Gpr::Rax, src: Gpr::Rcx });
                        Gpr::Rax
                    }
                    IBinOp::Or => {
                        code.push(MInst::Alu { op: AluOp::Or, dst: Gpr::Rax, src: Gpr::Rcx });
                        Gpr::Rax
                    }
                    IBinOp::Xor => {
                        code.push(MInst::Alu { op: AluOp::Xor, dst: Gpr::Rax, src: Gpr::Rcx });
                        Gpr::Rax
                    }
                    // The count is already in rcx (so in cl); shift rax by it.
                    IBinOp::Shl => {
                        code.push(MInst::Shift { op: ShiftOp::Shl, dst: Gpr::Rax });
                        Gpr::Rax
                    }
                    IBinOp::AShr => {
                        code.push(MInst::Shift { op: ShiftOp::Sar, dst: Gpr::Rax });
                        Gpr::Rax
                    }
                    _ => {
                        return Err(inst.span.clone().into_error(anyhow!(
                            "this integer op is not yet supported in codegen (TBD)"
                        )));
                    }
                };
                code.push(MInst::Store { disp: spill(*dst), src: result });
                Ok(())
            }
            // Compare operands in rax/rcx, materialize the 0/1 result: the flag
            // condition into al via setcc, then zero-extend it to the full slot.
            InstKind::ICmp { dst, pred, lhs, rhs, .. } => {
                self.load(lhs, Gpr::Rax, code)?;
                self.load(rhs, Gpr::Rcx, code)?;
                code.push(MInst::Alu { op: AluOp::Cmp, dst: Gpr::Rax, src: Gpr::Rcx });
                code.push(MInst::SetCC { cc: cc_of(*pred), dst: Gpr::Rax });
                code.push(MInst::Movzx8 { dst: Gpr::Rax });
                code.push(MInst::Store { disp: spill(*dst), src: Gpr::Rax });
                Ok(())
            }
            // A local read: copy the stack slot to the value's spill via rax.
            InstKind::Load { dst, slot, .. } => {
                code.push(MInst::Load { dst: Gpr::Rax, disp: self.local(*slot) });
                code.push(MInst::Store { disp: spill(*dst), src: Gpr::Rax });
                Ok(())
            }
            // A local write: the value reaches the stack slot via rax.
            InstKind::Store { slot, val, .. } => {
                self.load(val, Gpr::Rax, code)?;
                code.push(MInst::Store { disp: self.local(*slot), src: Gpr::Rax });
                Ok(())
            }
        }
    }

    fn term(&self, term: &Terminator, code: &mut Vec<MInst>) {
        match term {
            Terminator::Ret(v) => {
                if let Some(v) = v {
                    // The IR is well-typed by the time it reaches here, so a bad
                    // operand can't occur; ignore the (impossible) load error.
                    let _ = self.load(v, Gpr::Rax, code);
                }
                code.push(MInst::Leave);
                code.push(MInst::Ret);
            }
            Terminator::Br(bb) => code.push(MInst::Jmp(bb.0)),
            Terminator::CondBr { cond, then_bb, else_bb } => {
                // Branch to `then` when the condition is nonzero, else fall to `else`.
                let _ = self.load(cond, Gpr::Rax, code);
                code.push(MInst::Test { a: Gpr::Rax, b: Gpr::Rax });
                code.push(MInst::JmpCC { cc: Cc::Ne, target: then_bb.0 });
                code.push(MInst::Jmp(else_bb.0));
            }
        }
    }

    fn err(&self, msg: &str) -> Error {
        // Anchor at the function's first instruction if there is one.
        let span = self
            .func
            .blocks
            .iter()
            .flat_map(|b| b.insts.iter())
            .map(|i| i.span.clone())
            .next()
            .expect("codegen error only reachable with instructions present");
        span.into_error(anyhow!("{msg}"))
    }
}

// ----- encoding (BE-1) ------------------------------------------------------

#[derive(Default)]
struct Encoder {
    out: Vec<u8>,
    // Byte offset of each block label, indexed by block id.
    labels: Vec<usize>,
    // (rel32 byte position, target block id) for each jump awaiting a fixup.
    fixups: Vec<(usize, u32)>,
}

impl Encoder {
    fn b(&mut self, byte: u8) {
        self.out.push(byte);
    }

    fn le32(&mut self, v: i32) {
        self.out.extend_from_slice(&v.to_le_bytes());
    }

    /// A REX prefix with W set (64-bit operand), promoting the high bits of the
    /// `reg` and `rm` fields for r8..r15 (unused by the current register set).
    fn rex_w(&mut self, reg: u8, rm: u8) {
        let mut rex = 0x48; // 0100 1000 = REX.W
        if reg >= 8 {
            rex |= 0x04; // REX.R
        }
        if rm >= 8 {
            rex |= 0x01; // REX.B
        }
        self.b(rex);
    }

    fn modrm(&mut self, mode: u8, reg: u8, rm: u8) {
        self.b((mode << 6) | ((reg & 7) << 3) | (rm & 7));
    }

    /// A `[rbp + disp32]` memory operand. rbp (rm=101) is never the SIB escape,
    /// so no SIB byte; disp32 covers any frame offset.
    fn mem_rbp(&mut self, reg: u8, disp: i32) {
        self.modrm(0b10, reg, Gpr::Rbp.code());
        self.le32(disp);
    }

    fn encode(&mut self, inst: MInst) {
        match inst {
            MInst::PushRbp => self.b(0x55), // 50+rd, rd = rbp
            MInst::MovRbpRsp => {
                // mov rbp, rsp : 48 89 /r, reg=rsp, rm=rbp, mod=11
                self.rex_w(Gpr::Rsp.code(), Gpr::Rbp.code());
                self.b(0x89);
                self.modrm(0b11, Gpr::Rsp.code(), Gpr::Rbp.code());
            }
            MInst::SubRspImm(imm) => {
                // sub rsp, imm32 : 48 81 /5 id
                self.rex_w(0, Gpr::Rsp.code());
                self.b(0x81);
                self.modrm(0b11, 5, Gpr::Rsp.code());
                self.le32(imm);
            }
            MInst::Leave => self.b(0xC9),
            MInst::Ret => self.b(0xC3),
            MInst::MovRImm { dst, imm } => {
                if let Ok(imm32) = i32::try_from(imm) {
                    // mov r64, imm32 (sign-extended) : 48 C7 /0 id
                    self.rex_w(0, dst.code());
                    self.b(0xC7);
                    self.modrm(0b11, 0, dst.code());
                    self.le32(imm32);
                } else {
                    // movabs r64, imm64 : 48 B8+rd io
                    self.rex_w(0, dst.code());
                    self.b(0xB8 + (dst.code() & 7));
                    self.out.extend_from_slice(&imm.to_le_bytes());
                }
            }
            MInst::Load { dst, disp } => {
                // mov r64, [rbp+disp] : 48 8B /r
                self.rex_w(dst.code(), Gpr::Rbp.code());
                self.b(0x8B);
                self.mem_rbp(dst.code(), disp);
            }
            MInst::Store { disp, src } => {
                // mov [rbp+disp], r64 : 48 89 /r
                self.rex_w(src.code(), Gpr::Rbp.code());
                self.b(0x89);
                self.mem_rbp(src.code(), disp);
            }
            MInst::Alu { op, dst, src } => {
                // OP r/m64, r64 : 48 <op> /r, reg=src, rm=dst, mod=11
                self.rex_w(src.code(), dst.code());
                self.b(op.opcode());
                self.modrm(0b11, src.code(), dst.code());
            }
            MInst::IMul { dst, src } => {
                // imul r64, r/m64 : 48 0F AF /r, reg=dst, rm=src
                self.rex_w(dst.code(), src.code());
                self.b(0x0F);
                self.b(0xAF);
                self.modrm(0b11, dst.code(), src.code());
            }
            MInst::Cqo => {
                // cqo : 48 99
                self.b(0x48);
                self.b(0x99);
            }
            MInst::Idiv { src } => {
                // idiv r/m64 : 48 F7 /7
                self.rex_w(0, src.code());
                self.b(0xF7);
                self.modrm(0b11, 7, src.code());
            }
            MInst::Shift { op, dst } => {
                // OP r/m64, cl : 48 D3 /ext
                self.rex_w(0, dst.code());
                self.b(0xD3);
                self.modrm(0b11, op.ext(), dst.code());
            }
            MInst::SetCC { cc, dst } => {
                // setcc r/m8 : 0F (90+cc) /0 — no REX needed for al/cl/dl.
                self.b(0x0F);
                self.b(0x90 + cc.code());
                self.modrm(0b11, 0, dst.code());
            }
            MInst::Movzx8 { dst } => {
                // movzx r64, r/m8 : 48 0F B6 /r
                self.rex_w(dst.code(), dst.code());
                self.b(0x0F);
                self.b(0xB6);
                self.modrm(0b11, dst.code(), dst.code());
            }
            MInst::Movsxd { dst, src } => {
                // movsxd r64, r/m32 : 48 63 /r, reg=dst, rm=src
                self.rex_w(dst.code(), src.code());
                self.b(0x63);
                self.modrm(0b11, dst.code(), src.code());
            }
            MInst::Label(n) => {
                let n = n as usize;
                if n >= self.labels.len() {
                    self.labels.resize(n + 1, 0);
                }
                self.labels[n] = self.out.len();
            }
            MInst::Jmp(target) => {
                // jmp rel32 : E9 cd
                self.b(0xE9);
                self.fixups.push((self.out.len(), target));
                self.le32(0);
            }
            MInst::JmpCC { cc, target } => {
                // jcc rel32 : 0F 80+cc cd
                self.b(0x0F);
                self.b(0x80 + cc.code());
                self.fixups.push((self.out.len(), target));
                self.le32(0);
            }
            MInst::Test { a, b } => {
                // test r/m64, r64 : 48 85 /r, reg=b, rm=a
                self.rex_w(b.code(), a.code());
                self.b(0x85);
                self.modrm(0b11, b.code(), a.code());
            }
        }
    }

    /// Patch every recorded jump's rel32 to its now-known target label offset.
    /// rel32 is relative to the end of the jump instruction (`pos + 4`).
    fn resolve(&mut self) {
        for i in 0..self.fixups.len() {
            let (pos, target) = self.fixups[i];
            let rel = self.labels[target as usize] as i32 - (pos as i32 + 4);
            self.out[pos..pos + 4].copy_from_slice(&rel.to_le_bytes());
        }
    }
}

// ----- textual form (--dump-asm) -------------------------------------------

fn asm(inst: MInst) -> String {
    match inst {
        MInst::PushRbp => "push rbp".to_string(),
        MInst::MovRbpRsp => "mov rbp, rsp".to_string(),
        MInst::SubRspImm(imm) => format!("sub rsp, {imm}"),
        MInst::Leave => "leave".to_string(),
        MInst::Ret => "ret".to_string(),
        MInst::MovRImm { dst, imm } => format!("mov {}, {imm}", dst.name()),
        MInst::Load { dst, disp } => format!("mov {}, [rbp{disp:+}]", dst.name()),
        MInst::Store { disp, src } => format!("mov [rbp{disp:+}], {}", src.name()),
        MInst::Alu { op, dst, src } => format!("{} {}, {}", op.name(), dst.name(), src.name()),
        MInst::IMul { dst, src } => format!("imul {}, {}", dst.name(), src.name()),
        MInst::Cqo => "cqo".to_string(),
        MInst::Idiv { src } => format!("idiv {}", src.name()),
        MInst::Shift { op, dst } => format!("{} {}, cl", op.name(), dst.name()),
        MInst::SetCC { cc, dst } => format!("set{} {}", cc.name(), dst.name8()),
        MInst::Movzx8 { dst } => format!("movzx {}, {}", dst.name(), dst.name8()),
        MInst::Movsxd { dst, src } => format!("movsxd {}, {}", dst.name(), src.name32()),
        MInst::Label(n) => format!("bb{n}:"),
        MInst::Jmp(n) => format!("jmp bb{n}"),
        MInst::JmpCC { cc, target } => format!("j{} bb{target}", cc.name()),
        MInst::Test { a, b } => format!("test {}, {}", a.name(), b.name()),
    }
}

pub fn assembly(program: &ir::Program) -> Result<String> {
    let mut s = String::new();
    for func in &program.funcs {
        s.push_str(&format!("{}:\n", func.name));
        for inst in FuncSel::new(func).select()? {
            match inst {
                // Labels sit at the margin; everything else is indented.
                MInst::Label(n) => s.push_str(&format!("bb{n}:\n")),
                other => s.push_str(&format!("    {}\n", asm(other))),
            }
        }
    }
    Ok(s)
}

// ----- object emission (relocatable ELF) -----------------------------------

pub fn emit_object(program: &ir::Program) -> anyhow::Result<Vec<u8>> {
    let mut obj = Object::new(BinaryFormat::Elf, Architecture::X86_64, Endianness::Little);
    let text = obj.section_id(StandardSection::Text);

    for func in &program.funcs {
        let mut enc = Encoder::default();
        for inst in FuncSel::new(func).select()? {
            enc.encode(inst);
        }
        enc.resolve();
        let offset = obj.append_section_data(text, &enc.out, 16);

        obj.add_symbol(Symbol {
            name: func.name.clone().into_bytes(),
            value: offset,
            size: enc.out.len() as u64,
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
