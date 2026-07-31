//! Translation phase 4: the preprocessing driver. Owns the macro table and
//! runs directives + macro expansion over the pp-token stream. The phase 1–3
//! text handling and the tokenizer itself live in `lexer.rs`; the macro
//! machinery (definitions, hide sets, expansion) in `directive.rs`.

use std::{fs::File, io::Read, path::Path};

use anyhow::{Result, anyhow};

use super::{HideSet, PPKind, PPToken, TokenKind, directive, filter_esc_nl_and_rep_comments};
use crate::diagnostic::{SourceManager, Span};

/// a `#` that begins a line opens a directive
fn is_directive_start(t: &PPToken) -> bool {
    t.nl && t.kind == PPKind::Punct && t.text == "#"
}

/// One `#if`/`#ifdef`/`#ifndef` … `#endif` chain in flight.
struct Cond {
    /// tokens in the current branch reach the output
    active: bool,
    /// some branch already evaluated true — later `#elif`/`#else` stay dead
    taken: bool,
    /// the enclosing context was active; when false every branch is dead and
    /// no condition is even evaluated (6.10.1p7)
    parent_active: bool,
    /// `#else` was seen — further `#elif`/`#else` are malformed
    else_seen: bool,
    /// the opening directive, for the unterminated-conditional diagnostic
    span: Span,
}

pub struct Preprocessor<'a> {
    sm: &'a SourceManager,
    /// Living here (not threaded per call) is what lets an included file's
    /// definitions reach its includer across the recursive `run` calls.
    macros: directive::Macros,
}

impl<'a> Preprocessor<'a> {
    pub fn new(sm: &'a SourceManager) -> Self {
        Self { sm, macros: directive::Macros::default() }
    }

    /// Preprocess `path` and everything it includes into one pp-token stream.
    pub fn process(mut self, path: impl AsRef<Path>) -> Result<Vec<PPToken>> {
        // can't be None cause that's just if we've opened it before...
        let content = self.open(path)?.unwrap();
        self.run(&content)
    }

    /// Translation phases 1–3 for one file: read it, register it with the
    /// source manager, splice escaped newlines, replace comments with spaces.
    /// `None` means the file was already opened before.
    ///
    /// The chars borrow from the `SourceManager` (`'a`), not from `self` —
    /// which is what lets `run` recurse mutably while a file's content is
    /// still alive.
    fn open(&self, path: impl AsRef<Path>) -> Result<Option<Vec<&'a char>>> {
        let path = path.as_ref().to_path_buf();
        if !self.sm.is_opened(&path) {
            let mut f = File::open(&path)?;
            let mut buf = String::new();
            f.read_to_string(&mut buf)?;

            // phase 1
            let iter = self
                .sm
                .add_source(buf.chars().collect::<Vec<_>>().into_boxed_slice(), path);

            // phase 2 and partial phase 3, replacing comments with spaces
            let vec = filter_esc_nl_and_rep_comments(iter);

            return Ok(Some(vec));
        }

        Ok(None)
    }

    /// Translation phase 4: execute directives and expand macros, in one
    /// left-to-right scan (so definitions apply only to uses after them, and
    /// `#undef` takes effect mid-file). Conditional groups must balance
    /// within the file: the `Cond` stack is per `run` call.
    fn run(&mut self, content: &[&'a char]) -> Result<Vec<PPToken>> {
        let mut tokens = self.pp_tokenize(content);

        let mut conds: Vec<Cond> = Vec::new();
        let mut i = 0;

        while i < tokens.len() {
            let active = conds.last().is_none_or(|c| c.active);
            if is_directive_start(&tokens[i]) {
                // the directive line runs to the next token that starts a line
                let eol = tokens[i + 1..]
                    .iter()
                    .position(|x| x.nl)
                    .map_or(tokens.len(), |n| i + 1 + n);
                // TODO: make last token {#} an error earlier
                match &tokens[i + 1..] {
                    // conditionals are processed even in skipped groups (they
                    // nest); everything *else* in a skipped group is dropped
                    // below without being looked at (6.10.1p7)
                    [PPToken { text, kind: PPKind::Ident, nl: false, .. }, ..]
                        if matches!(
                            text.as_str(),
                            "if" | "ifdef" | "ifndef" | "elif" | "else" | "endif"
                        ) =>
                    {
                        self.conditional(&mut conds, &tokens[i + 1..eol])?;
                        tokens.drain(i..eol);
                    }
                    _ if !active => {
                        tokens.drain(i..eol);
                    }
                    // TODO: make include w/o header-name error
                    [
                        PPToken { text, kind: PPKind::Ident, nl: false, .. },
                        PPToken { text: path, kind: PPKind::Header, .. },
                        ..,
                    ] if text.as_str() == "include" => {
                        let mut chars = path.chars();
                        chars.next();
                        chars.next_back();
                        let path = chars.as_str();
                        if let Some(content) = self.open(path)? {
                            let more_tokens = self.run(&content)?;
                            tokens.splice(i..i + 3, more_tokens.into_iter());
                        } else {
                            // already opened once: splice in nothing (a crude
                            // implicit `#pragma once`), but still drop the
                            // directive so the scan makes progress
                            tokens.drain(i..i + 3);
                        }
                    }
                    // TODO: make define w/o identifier error
                    [
                        PPToken { text, kind: PPKind::Ident, nl: false, .. },
                        PPToken { text: key, kind: PPKind::Ident, nl: false, .. },
                        ..,
                    ] if text.as_str() == "define" => {
                        // the body is everything after the name to end of line
                        let body_at = i + 3;
                        let end = tokens[body_at..]
                            .iter()
                            .position(|x| x.nl)
                            .map_or(tokens.len(), |eol| body_at + eol);
                        let def = directive::DefineFn::new(&tokens[body_at..end])?;
                        // TODO: 6.10.5p2 — a redefinition must be identical;
                        // diagnose instead of overwriting
                        self.macros.insert(key.clone(), def);
                        tokens.drain(i..end);
                    }
                    [
                        PPToken { text, kind: PPKind::Ident, nl: false, .. },
                        PPToken { text: key, kind: PPKind::Ident, nl: false, .. },
                        ..,
                    ] if text.as_str() == "undef" => {
                        self.macros.remove(key);
                        tokens.drain(i..i + 3);
                    }
                    x => {
                        todo!("TODO: {:?}", x);
                        // i += 1;
                    }
                };
            } else if !active {
                // skipped group: drop everything up to the next directive line
                let end = tokens[i + 1..]
                    .iter()
                    .position(|t| is_directive_start(t))
                    .map_or(tokens.len(), |n| i + 1 + n);
                tokens.drain(i..end);
            } else if !directive::try_expand_at(&mut tokens, i, &self.macros)? {
                i += 1;
            }
            // when try_expand_at expanded, stay at `i` and rescan the splice
        }

        if let Some(c) = conds.last() {
            return Err(c
                .span
                .clone()
                .into_error(anyhow!("unterminated conditional directive"))
                .into());
        }

        Ok(tokens)
    }

    /// Process one conditional directive line (`line[0]` is the directive
    /// name, the rest its operands) against the `Cond` stack.
    fn conditional(&self, conds: &mut Vec<Cond>, line: &[PPToken]) -> Result<()> {
        let name = &line[0];
        let rest = &line[1..];
        match name.text.as_str() {
            "if" | "ifdef" | "ifndef" => {
                let parent_active = conds.last().is_none_or(|c| c.active);
                // a group inside a skipped group is nested but never evaluated
                let value = if parent_active {
                    self.branch_value(name, rest)?
                } else {
                    false
                };
                conds.push(Cond {
                    active: value,
                    taken: value,
                    parent_active,
                    else_seen: false,
                    span: name.span.clone(),
                });
            }
            "elif" => {
                let Some(c) = conds.last_mut() else {
                    return Err(name.err("`#elif` without `#if`").into());
                };
                if c.else_seen {
                    return Err(name.err("`#elif` after `#else`").into());
                }
                let value = if c.parent_active && !c.taken {
                    self.eval_condition(name, rest)?
                } else {
                    false
                };
                c.active = value;
                c.taken |= value;
            }
            "else" => {
                let Some(c) = conds.last_mut() else {
                    return Err(name.err("`#else` without `#if`").into());
                };
                if c.else_seen {
                    return Err(name.err("duplicate `#else`").into());
                }
                if let [extra, ..] = rest {
                    return Err(extra.err("extra tokens after `#else`").into());
                }
                c.else_seen = true;
                c.active = c.parent_active && !c.taken;
                c.taken = true;
            }
            "endif" => {
                if conds.pop().is_none() {
                    return Err(name.err("`#endif` without `#if`").into());
                }
                if let [extra, ..] = rest {
                    return Err(extra.err("extra tokens after `#endif`").into());
                }
            }
            _ => unreachable!("caller matched the directive name"),
        }
        Ok(())
    }

    /// The opening branch's truth: `#if expr`, or `#ifdef` / `#ifndef` NAME.
    fn branch_value(&self, name: &PPToken, rest: &[PPToken]) -> Result<bool> {
        if name.text == "if" {
            return self.eval_condition(name, rest);
        }
        let [PPToken { text, kind: PPKind::Ident, .. }] = rest else {
            let msg = format!("expected exactly one identifier after `#{}`", name.text);
            return Err(name.err(msg).into());
        };
        let defined = self.macros.contains_key(text);
        Ok(if name.text == "ifdef" {
            defined
        } else {
            !defined
        })
    }

    /// Evaluate an `#if`/`#elif` controlling expression: resolve `defined`
    /// *before* macro expansion (6.10.1p11), expand what remains, then fold
    /// the line as an integer constant expression.
    fn eval_condition(&self, name: &PPToken, rest: &[PPToken]) -> Result<bool> {
        if rest.is_empty() {
            return Err(name
                .err(format!("`#{}` with no expression", name.text))
                .into());
        }
        let mut line = rest.to_vec();
        self.resolve_defined(&mut line)?;
        directive::expand(&mut line, &self.macros)?;
        let value = Eval { tokens: &line, pos: 0, anchor: name }.parse()?;
        Ok(value.truthy())
    }

    /// Replace `defined NAME` / `defined ( NAME )` with `1`/`0`. Running
    /// before macro expansion is what keeps the operand unexpanded.
    fn resolve_defined(&self, line: &mut Vec<PPToken>) -> Result<()> {
        let mut i = 0;
        while i < line.len() {
            if !(line[i].kind == PPKind::Ident && line[i].text == "defined") {
                i += 1;
                continue;
            }
            let (name, used) = match &line[i + 1..] {
                [PPToken { text, kind: PPKind::Ident, .. }, ..] => (text, 2),
                [
                    PPToken { text: l, kind: PPKind::Punct, .. },
                    PPToken { text, kind: PPKind::Ident, .. },
                    PPToken { text: r, kind: PPKind::Punct, .. },
                    ..,
                ] if l == "(" && r == ")" => (text, 4),
                _ => return Err(line[i].err("expected an identifier after `defined`").into()),
            };
            let value = if self.macros.contains_key(name) {
                "1"
            } else {
                "0"
            };
            let tok = PPToken {
                text: value.to_string(),
                kind: PPKind::PPNumber,
                nl: false,
                ws: line[i].ws,
                span: line[i].span.clone(),
                hs: HideSet::default(),
            };
            line.splice(i..i + used, std::iter::once(tok));
            i += 1;
        }
        Ok(())
    }

    fn pp_tokenize(&self, s: &[&char]) -> Vec<PPToken> {
        let (src, content) = self.sm.latest();
        super::pp_tokenize(s, content, src)
    }
}

/// An `#if` value: 6.10.1p6 does all arithmetic in `intmax_t`/`uintmax_t`
/// (i64/u64 here), and the usual arithmetic conversions at that width reduce
/// to "unsigned wins".
#[derive(Clone, Copy)]
enum PPInt {
    Signed(i64),
    Unsigned(u64),
}

impl PPInt {
    fn truthy(self) -> bool {
        self.bits() != 0
    }

    /// the raw two's-complement bits — the representation both signednesses
    /// share, so most operators can work on it directly
    fn bits(self) -> u64 {
        match self {
            PPInt::Signed(v) => v as u64,
            PPInt::Unsigned(v) => v,
        }
    }

    fn is_unsigned(self) -> bool {
        matches!(self, PPInt::Unsigned(_))
    }

    /// tag `bits` with a signedness
    fn of(unsigned: bool, bits: u64) -> PPInt {
        if unsigned {
            PPInt::Unsigned(bits)
        } else {
            PPInt::Signed(bits as i64)
        }
    }
}

/// C operator precedence for the `#if` grammar (a conditional-expression, so
/// no comma or assignment), as Pratt binding powers. `?:` is right-
/// associative and handled specially in `Eval::expr`.
fn infix_bp(op: &str) -> Option<u8> {
    Some(match op {
        "?" => 1,
        "||" => 2,
        "&&" => 3,
        "|" => 4,
        "^" => 5,
        "&" => 6,
        "==" | "!=" => 7,
        "<" | ">" | "<=" | ">=" => 8,
        "<<" | ">>" => 9,
        "+" | "-" => 10,
        "*" | "/" | "%" => 11,
        _ => return None,
    })
}

/// tighter than every infix operator: a unary operand is one atom
const UNARY_BP: u8 = 12;

/// A Pratt parser/folder for `#if` controlling expressions, run over the
/// already-`defined`-resolved, macro-expanded directive line.
struct Eval<'t> {
    tokens: &'t [PPToken],
    pos: usize,
    /// the directive-name token — the error anchor when the line ends early
    /// (the line itself can expand to nothing)
    anchor: &'t PPToken,
}

impl<'t> Eval<'t> {
    fn parse(mut self) -> Result<PPInt> {
        let value = self.expr(0, true)?;
        if let Some(t) = self.peek() {
            return Err(t.err("unexpected token in `#if` expression").into());
        }
        Ok(value)
    }

    fn peek(&self) -> Option<&'t PPToken> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<&'t PPToken> {
        let t = self.tokens.get(self.pos);
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn end_err(&self) -> anyhow::Error {
        self.anchor.err("`#if` expression ends unexpectedly").into()
    }

    fn expect(&mut self, p: &str) -> Result<()> {
        match self.next() {
            Some(t) if t.kind == PPKind::Punct && t.text == p => Ok(()),
            Some(t) => Err(t.err(format!("expected `{p}` in `#if` expression")).into()),
            None => Err(self.end_err()),
        }
    }

    /// Fold an expression whose operators all bind at least as tightly as
    /// `min_bp`. `live` is false inside unevaluated operands (the untaken arm
    /// of `?:`, the short-circuited side of `&&`/`||`): parsing continues but
    /// value errors like division by zero must not fire there.
    fn expr(&mut self, min_bp: u8, live: bool) -> Result<PPInt> {
        let mut lhs = self.atom(live)?;

        while let Some(t) = self.peek() {
            let bp = if t.kind == PPKind::Punct {
                infix_bp(&t.text)
            } else {
                None
            };
            let Some(bp) = bp else { break };
            if bp < min_bp {
                break;
            }
            self.pos += 1;

            lhs = match t.text.as_str() {
                "?" => {
                    let cond = lhs.truthy();
                    let mid = self.expr(0, live && cond)?;
                    self.expect(":")?;
                    // right-associative: the else-arm reparses at the same bp
                    let els = self.expr(bp, live && !cond)?;
                    if cond { mid } else { els }
                }
                "||" => {
                    let rhs = self.expr(bp + 1, live && !lhs.truthy())?;
                    PPInt::Signed((lhs.truthy() || rhs.truthy()) as i64)
                }
                "&&" => {
                    let rhs = self.expr(bp + 1, live && lhs.truthy())?;
                    PPInt::Signed((lhs.truthy() && rhs.truthy()) as i64)
                }
                _ => {
                    let rhs = self.expr(bp + 1, live)?;
                    binop(t, lhs, rhs, live)?
                }
            };
        }

        Ok(lhs)
    }

    fn atom(&mut self, live: bool) -> Result<PPInt> {
        let Some(t) = self.next() else {
            return Err(self.end_err());
        };

        if t.kind == PPKind::Punct {
            return match t.text.as_str() {
                "(" => {
                    let value = self.expr(0, live)?;
                    self.expect(")")?;
                    Ok(value)
                }
                "+" => self.expr(UNARY_BP, live),
                "-" => {
                    let v = self.expr(UNARY_BP, live)?;
                    Ok(PPInt::of(v.is_unsigned(), v.bits().wrapping_neg()))
                }
                "!" => Ok(PPInt::Signed(!self.expr(UNARY_BP, live)?.truthy() as i64)),
                "~" => {
                    let v = self.expr(UNARY_BP, live)?;
                    Ok(PPInt::of(v.is_unsigned(), !v.bits()))
                }
                _ => Err(t.err("expected a value in `#if` expression").into()),
            };
        }

        match t.kind {
            PPKind::PPNumber => match t.convert_pp_number()? {
                TokenKind::IntConst { value, suf } => {
                    // an unsuffixed constant above intmax range lives as
                    // uintmax: exact for hex/octal (6.4.4.1's ladder); for
                    // decimal C calls it untypable, and we take gcc's reading
                    Ok(PPInt::of(suf.unsigned || value > i64::MAX as u64, value))
                }
                _ => Err(t.err("floating constant in `#if` expression").into()),
            },
            PPKind::CharConst => match t.convert_char_const()? {
                TokenKind::CharConst { value, .. } => Ok(PPInt::Signed(value as i64)),
                _ => unreachable!("convert_char_const yields CharConst"),
            },
            PPKind::Ident => match t.text.as_str() {
                // 6.10.1p6: boolean literals; p13: every identifier that
                // survives expansion evaluates as 0
                "true" => Ok(PPInt::Signed(1)),
                "false" => Ok(PPInt::Signed(0)),
                "defined" => Err(t.err("`defined` produced by macro expansion").into()),
                "__has_include" | "__has_embed" | "__has_c_attribute" => {
                    Err(t.err(format!("`{}` is TBD", t.text)).into())
                }
                _ => Ok(PPInt::Signed(0)),
            },
            _ => Err(t.err("unexpected token in `#if` expression").into()),
        }
    }
}

/// Apply a binary operator under the usual arithmetic conversions at intmax
/// width: if either side is unsigned the operation is unsigned. Shifts take
/// the *left* operand's signedness (integer promotion, not conversion), and
/// comparisons yield signed 0/1. When `live` is false the operands come from
/// an unevaluated context: return a dummy without diagnosing.
fn binop(op: &PPToken, l: PPInt, r: PPInt, live: bool) -> Result<PPInt> {
    if !live {
        return Ok(PPInt::Signed(0));
    }

    let unsigned = l.is_unsigned() || r.is_unsigned();

    // two's-complement wrapping arithmetic is signedness-agnostic on the raw
    // bits; only division, right shift, and ordering have to split
    Ok(match op.text.as_str() {
        "+" => PPInt::of(unsigned, l.bits().wrapping_add(r.bits())),
        "-" => PPInt::of(unsigned, l.bits().wrapping_sub(r.bits())),
        "*" => PPInt::of(unsigned, l.bits().wrapping_mul(r.bits())),
        "/" | "%" => {
            if r.bits() == 0 {
                return Err(op.err("division by zero in `#if` expression").into());
            }
            let bits = match (op.text.as_str(), unsigned) {
                ("/", true) => l.bits() / r.bits(),
                ("/", false) => (l.bits() as i64).wrapping_div(r.bits() as i64) as u64,
                ("%", true) => l.bits() % r.bits(),
                _ => (l.bits() as i64).wrapping_rem(r.bits() as i64) as u64,
            };
            PPInt::of(unsigned, bits)
        }
        "&" => PPInt::of(unsigned, l.bits() & r.bits()),
        "|" => PPInt::of(unsigned, l.bits() | r.bits()),
        "^" => PPInt::of(unsigned, l.bits() ^ r.bits()),
        "<<" | ">>" => {
            if (!r.is_unsigned() && (r.bits() as i64) < 0) || r.bits() >= 64 {
                return Err(op
                    .err("shift count out of range in `#if` expression")
                    .into());
            }
            let n = r.bits() as u32;
            let bits = match (op.text.as_str(), l.is_unsigned()) {
                ("<<", _) => l.bits() << n,
                (_, true) => l.bits() >> n,
                // arithmetic right shift for signed values
                _ => ((l.bits() as i64) >> n) as u64,
            };
            PPInt::of(l.is_unsigned(), bits)
        }
        cmp => {
            let ord = if unsigned {
                l.bits().cmp(&r.bits())
            } else {
                (l.bits() as i64).cmp(&(r.bits() as i64))
            };
            let value = match cmp {
                "==" => ord.is_eq(),
                "!=" => ord.is_ne(),
                "<" => ord.is_lt(),
                ">" => ord.is_gt(),
                "<=" => ord.is_le(),
                ">=" => ord.is_ge(),
                _ => unreachable!("infix_bp gates the operator set"),
            };
            PPInt::Signed(value as i64)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run the full phase-1..4 pipeline (splice, comments, tokenize,
    /// directives + macro expansion) over a source string.
    fn try_preprocess_str(s: &str) -> Result<Vec<PPToken>> {
        let sm = SourceManager::new();
        let iter = sm.add_source(
            s.chars().collect::<Vec<_>>().into_boxed_slice(),
            std::path::PathBuf::from("test.c"),
        );
        let content = filter_esc_nl_and_rep_comments(iter);
        Preprocessor::new(&sm).run(&content)
    }

    fn preprocess_str(s: &str) -> Vec<PPToken> {
        try_preprocess_str(s).unwrap()
    }

    fn texts(tokens: &[PPToken]) -> Vec<&str> {
        tokens.iter().map(|t| t.text.as_str()).collect()
    }

    #[test]
    fn define_and_expand() {
        let result = preprocess_str(
            "#define TIMES(a, b) ((a) * (b))\n#define SIX TIMES(2, 3)\nint x = SIX;\n",
        );

        #[rustfmt::skip]
        assert_eq!(
            texts(&result),
            ["int", "x", "=", "(", "(", "2", ")", "*", "(", "3", ")", ")", ";"]
        );
    }

    #[test]
    fn define_undef() {
        let result = preprocess_str("#define A 1\nA\n#undef A\nA\n");
        assert_eq!(texts(&result), ["1", "A"]);
    }

    #[test]
    fn define_terminates() {
        // hide sets, end to end: the painted survivor stays put
        let result = preprocess_str("#define x x\nx\n");
        assert_eq!(texts(&result), ["x"]);
        assert!(result[0].hs.hides("x"));
    }

    #[test]
    fn ifdef_and_ifndef() {
        let src = "#define A\n#ifdef A\nyes\n#endif\n#ifdef B\nno\n#endif\n";
        assert_eq!(texts(&preprocess_str(src)), ["yes"]);
        assert_eq!(texts(&preprocess_str("#ifndef B\nyes\n#endif\n")), ["yes"]);
    }

    #[test]
    fn include_guard() {
        // the classic pattern, twice over: the second copy is fully skipped
        let src = "#ifndef H\n#define H\nint x;\n#endif\n\
                   #ifndef H\nint y;\n#endif\n";
        assert_eq!(texts(&preprocess_str(src)), ["int", "x", ";"]);
    }

    #[test]
    fn elif_chains() {
        let src = "#if 0\na\n#elif 0\nb\n#elif 1\nc\n#else\nd\n#endif\n";
        assert_eq!(texts(&preprocess_str(src)), ["c"]);
        // once a branch is taken, later true branches stay dead
        let src = "#if 1\na\n#elif 1\nb\n#else\nc\n#endif\n";
        assert_eq!(texts(&preprocess_str(src)), ["a"]);
        let src = "#if 0\na\n#else\nb\n#endif\n";
        assert_eq!(texts(&preprocess_str(src)), ["b"]);
    }

    #[test]
    fn skipped_groups_are_inert() {
        // 6.10.1p7: nothing in a dead branch is evaluated, expanded, defined,
        // or even understood — only the conditional nesting is tracked
        let src = "#if 0\n\
                   #pragma junk\n\
                   #error nope\n\
                   #define X 1\n\
                   1/0\n\
                   #if UNDEFINED_JUNK / 0\ndeep\n#elif 1\ndeeper\n#endif\n\
                   #endif\n\
                   X\n";
        assert_eq!(texts(&preprocess_str(src)), ["X"]);
    }

    #[test]
    fn if_evaluator() {
        for (e, want) in [
            ("1 + 2 * 3 == 7", true),
            ("(1 + 2) * 3 == 7", false),
            ("10 % 4 == 2 && 10 / 4 == 2", true),
            // shifts are left-associative and bind tighter than comparison
            ("1 << 2 << 3 == 32", true),
            ("-1 < 0", true),
            // usual arithmetic conversions: -1 converts to huge unsigned
            ("-1 < 0u", false),
            ("-1 / 2u == 0x7FFFFFFFFFFFFFFF", true),
            ("0xFFFFFFFFFFFFFFFF == -1", true),
            ("-7 / 2 == -3 && -7 % 2 == -1", true),
            ("~0 == -1", true),
            ("'A' == 65", true),
            ("!0 && !!5", true),
            // untaken arms and short-circuited sides are not evaluated
            ("0 ? 1/0 : 2", true),
            ("1 || 1/0", true),
            ("0 && 1/0", false),
            // ?: is right-associative: 1 ? 0 : (…), not (1 ? 0 : 1) ? 1 : 1
            ("1 ? 0 : 1 ? 1 : 1", false),
            // == binds tighter than |
            ("(2 | 1 == 3) == 2", true),
            ("true && !false", true),
            // 6.10.1p13: unknown identifiers evaluate as 0
            ("garbage + 1 == 1", true),
        ] {
            let r = try_preprocess_str(&format!("#if {e}\nT\n#endif\n"))
                .unwrap_or_else(|err| panic!("`#if {e}` errored: {err}"));
            assert_eq!(!r.is_empty(), want, "`#if {e}` should be {want}");
        }
    }

    #[test]
    fn if_defined_and_macros() {
        let src = "#define FOO 10\n\
                   #if defined FOO && FOO > 5\na\n#endif\n\
                   #if defined(BAR) || FOO == 10\nb\n#endif\n\
                   #if !defined BAR && !defined(FOO)\nc\n#endif\n\
                   #undef FOO\n\
                   #if defined FOO\nd\n#endif\n";
        assert_eq!(texts(&preprocess_str(src)), ["a", "b"]);
    }

    #[test]
    fn cond_errors() {
        for src in [
            "#endif\n",
            "#else\n",
            "#elif 1\n",
            "#if 1\n#else\n#else\n#endif\n",
            "#if 1\n#else\n#elif 0\n#endif\n",
            "#if 1\n",
            "#if 1\n#endif junk\n",
            "#if 0\n#else junk\n#endif\n",
            "#ifdef\n#endif\n",
            "#ifdef A B\n#endif\n",
            "#if\n#endif\n",
            "#if 1/0\n#endif\n",
            "#if (1\n#endif\n",
            "#if 1 +\n#endif\n",
            "#if 1 2\n#endif\n",
            "#if 1 , 2\n#endif\n",
            "#if defined\n#endif\n",
            "#if defined(A\n#endif\n",
            "#if 1.5\n#endif\n",
            "#if \"s\"\n#endif\n",
            "#if 1 << 64\n#endif\n",
            "#if __has_include(<stdio.h>)\n#endif\n",
        ] {
            assert!(
                try_preprocess_str(src).is_err(),
                "`{}` should be rejected",
                src.escape_debug()
            );
        }
    }

    /// C23 6.10.5.5 EXAMPLE 3 (the parts without `#`/`##`) — the canonical
    /// hide-set torture test: self-reference (`f`, `z`), reference through
    /// other macros (`g` → `f`, `t`, `m(m)`), a body with an unbalanced paren
    /// (`h`), commas produced by expansion *not* splitting arguments (`w`),
    /// invocations formed across splice boundaries (`h 5)`, `t(...)(1)`), and
    /// lazy bodies observing `#undef`/redefinition (`x`).
    ///
    /// The standard prints the expected expansion:
    /// ```c
    /// f(2 * (y+1)) + f(2 * (f(2 * (z[0])))) % f(2 * (0)) + t(1);
    /// f(2 * (2+(3,4)-0,1)) | f(2 * (~ 5)) & f(2 * (0,1))^m(0,1);
    /// ```
    ///
    /// The snapshot holds the rendered token stream (nl/ws-flag spacing, so
    /// whitespace is approximate) followed by every token with its hide set.
    #[test]
    fn expansion_torture() {
        let result = preprocess_str(
            "#define x 3\n\
             #define f(a) f(x * (a))\n\
             #undef x\n\
             #define x 2\n\
             #define g f\n\
             #define z z[0]\n\
             #define h g(~\n\
             #define m(a) a(w)\n\
             #define w 0,1\n\
             #define t(a) a\n\
             f(y+1) + f(f(z)) % t(t(g)(0) + t)(1);\n\
             g(x+(3,4)-w) | h 5) & m(f)^m(m);\n",
        );

        let mut rendered = String::new();
        for t in &result {
            if t.nl {
                rendered.push('\n');
            } else if t.ws {
                rendered.push(' ');
            }
            rendered.push_str(&t.text);
        }

        let listing: Vec<String> = result.iter().map(|t| format!("{t:?}")).collect();
        insta::assert_snapshot!(format!(
            "{}\n\n{}",
            rendered.trim_start(),
            listing.join("\n")
        ));
    }
}
