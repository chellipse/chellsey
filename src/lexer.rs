/// Does not currently handle [UAX #31](https://www.unicode.org/reports/tr31/)
use std::{fs::File, io::Read, iter::Peekable, path::Path};

use anyhow::{Result, anyhow};
use constcat::concat_slices;

mod dfa;
mod directive;
mod token;

pub use token::*;

use crate::diagnostic::{SourceManager, Span};

// 6.4.2

const LOWER: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
    't', 'u', 'v', 'w', 'x', 'y', 'z',
];

const UPPER: &[char] = &[
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S',
    'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
];

const NON_DIGIT: &[char] = concat_slices!([char]: &['_'], LOWER, UPPER);

const DIGIT: &[char] = &['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];

const DIGIT_AND_NON_DIGIT: &[char] = concat_slices!([char]: NON_DIGIT, DIGIT);

// TODO: Missing XID_Continue and UCN of class XID_Continue
const IDENT_CONT: &[char] = DIGIT_AND_NON_DIGIT;

const SIMPLE_ESCAPES: &[char] = &['\'', '"', '?', '\\', 'a', 'b', 'f', 'n', 'r', 't', 'v'];
const OCTAL: &[char] = &['0', '1', '2', '3', '4', '5', '6', '7'];
// TODO: compare asm from HEX.contains(x) with is_ascii_hexdigit, is_digit, etc
const HEX: &[char] = &[
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'A', 'B', 'C',
    'D', 'E', 'F',
];

const VA_ARGS: &str = "__VA_ARGS__";
const VA_OPT: &str = "__VA_OPT__";
const LPAREN: &str = "(";
const RPAREN: &str = ")";
const COMMA: &str = ",";
const SPACE: &char = &' ';

pub struct Lexer<'a> {
    sm: &'a SourceManager,
}

impl<'a> Lexer<'a> {
    pub fn new(sm: &'a SourceManager) -> Self {
        Self { sm }
    }

    fn open(&self, path: impl AsRef<Path>) -> Result<Option<Vec<&char>>> {
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

    pub fn lex(self, path: impl AsRef<Path>) -> Result<Vec<Token>> {
        // can't be None cause that's just if we've opened it before...
        let content = self.open(path)?.unwrap();

        let mut macros = directive::Macros::default();
        let tokens = self.preprocess(&content, &mut macros)?;
        println!("PPTokens: {:?}", &tokens);

        // Phase 5 & 6 ignored

        let mut result = Vec::with_capacity(tokens.len());

        for token in tokens {
            // `promote` yields a span-carrying `diagnostic::Error`; `?` folds it
            // into `anyhow`, and `main` resolves it against `SOURCES`.
            result.push(token.promote()?);
        }

        Ok(result)
    }

    /// Translation phase 4: execute directives and expand macros, in one
    /// left-to-right scan (so definitions apply only to uses after them, and
    /// `#undef` takes effect mid-file). `macros` is threaded through
    /// `#include` recursion so an included file's definitions reach its
    /// includer.
    fn preprocess(
        &self,
        content: &[&char],
        macros: &mut directive::Macros,
    ) -> Result<Vec<PPToken>> {
        let mut tokens = self.pp_tokenize(&content);

        let mut i = 0;

        while i < tokens.len() {
            if let PPToken { text, kind: PPKind::Punct, nl: true, .. } = &tokens[i]
                && text.as_str() == "#"
            {
                // TODO: make last token {#} an error earlier
                match &tokens[i + 1..] {
                    // TODO: make include w/o header-name error
                    [
                        PPToken { text, kind: PPKind::Ident, .. },
                        PPToken { text: path, kind: PPKind::Header, .. },
                        ..,
                    ] if text.as_str() == "include" => {
                        let mut chars = path.chars();
                        chars.next();
                        chars.next_back();
                        let path = chars.as_str();
                        if let Some(content) = self.open(path)? {
                            let more_tokens = self.preprocess(&content, macros)?;
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
                        PPToken { text, kind: PPKind::Ident, .. },
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
                        macros.insert(key.clone(), def);
                        tokens.drain(i..end);
                    }
                    [
                        PPToken { text, kind: PPKind::Ident, .. },
                        PPToken { text: key, kind: PPKind::Ident, nl: false, .. },
                        ..,
                    ] if text.as_str() == "undef" => {
                        macros.remove(key);
                        tokens.drain(i..i + 3);
                    }
                    x => {
                        todo!("TODO: {:?}", x);
                        // i += 1;
                    }
                };
            } else if !directive::try_expand_at(&mut tokens, i, macros)? {
                i += 1;
            }
            // when try_expand_at expanded, stay at `i` and rescan the splice
        }

        Ok(tokens)
    }

    fn pp_tokenize(&self, s: &[&char]) -> Vec<PPToken> {
        let (src, content) = self.sm.latest();
        pp_tokenize(s, content, src)
    }
}

fn filter_esc_nl_and_rep_comments<'a>(
    mut iter: Peekable<impl Iterator<Item = &'a char>>,
) -> Vec<&'a char> {
    let mut vec = Vec::new();
    loop {
        match (iter.next(), iter.peek()) {
            (Some('\\'), Some('\n')) => {
                iter.next();
            }
            (Some(c), _) => {
                vec.push(c);
            }
            (None, _) => break,
        }
    }

    // partial phase 3, replacing comments with spaces
    let mut i = 0;
    while i < vec.len() {
        if let Some(l) = dfa::dfa_comment(&vec[i..]).ok() {
            vec.splice(i..i + l, std::iter::once(SPACE));
        } else {
            i += 1;
        }
    }

    vec
}

/// None indicates that `item` was not within `slice`
fn get_index<T>(slice: &[T], item: &T) -> Option<usize> {
    let rbase = slice.as_ptr() as usize;
    let raddr = item as *const _ as usize;

    let offset = raddr.checked_sub(rbase)? / std::mem::size_of::<T>();

    if offset >= slice.len() {
        None
    } else {
        Some(offset)
    }
}

fn get_span(slice: &[&char], content: &[char], src: usize) -> Result<Span> {
    let start = get_index(content, slice[0]).ok_or(anyhow!("`a` not within content"))?;
    let end =
        get_index(content, slice[slice.len() - 1]).ok_or(anyhow!("`b` not within content"))?;

    Ok(Span { start, end, src })
}

fn pp_tokenize(s: &[&char], content: &[char], src: usize) -> Vec<PPToken> {
    let mut i = 0;
    let mut options: Vec<(usize, PPKind)> = Vec::new();
    let mut result: Vec<PPToken> = Vec::new();
    let mut nl = true;
    let mut ws = true;

    while i < s.len() {
        let slice = &s[i..];

        // TODO: need more line state here
        if let [
            ..,
            PPToken { text: pt, kind: PPKind::Punct, nl: true, .. },
            PPToken { text: it, kind: PPKind::Ident, .. },
        ] = result.as_slice()
            && pt == "#"
            && matches!(it.as_str(), "include" | "embed")
        {
            if let Some(l) = dfa::dfa_header_name(slice).ok() {
                options.push((l, PPKind::Header));
            }
        }

        if let Some(l) = dfa::dfa_identifier(slice).ok() {
            options.push((l, PPKind::Ident));
        }

        if let Some(l) = dfa::dfa_pp_number(slice).ok() {
            options.push((l, PPKind::PPNumber));
        }

        if let Some(l) = dfa::dfa_character_constant(slice).ok() {
            options.push((l, PPKind::CharConst));
        }

        if let Some(l) = dfa::dfa_string_literal(slice).ok() {
            options.push((l, PPKind::StrLit));
        }

        if let Some(l) = dfa::dfa_punctuator(slice).ok() {
            options.push((l, PPKind::Punct));
        }

        if let Some(l) = dfa::dfa_ucn(slice).ok() {
            options.push((l, PPKind::UCN));
        }

        if let Some(l) = dfa::dfa_other(slice).ok() {
            options.push((l, PPKind::Other));
        }

        // maximal munch, no token precedence aside from
        // header-name / string-literal as described in 6.4 paragraph 4
        // the header-name > string-literal precedence comes from initial options
        // order, since sort is stable
        options.sort_by(|a, b| b.0.cmp(&a.0));
        if let Some(last) = options.get(0) {
            let len = last.0;
            let kind = last.1;
            let char_text = &slice[..len];
            let span = get_span(char_text, content, src).unwrap();
            let text: String = char_text.into_iter().cloned().collect();
            i += len;
            result.push(PPToken { text, kind, nl, ws, span, hs: HideSet::default() });
            // println!("{:?} {:?}", result.last().unwrap(), kind);
            nl = false;
            ws = false;
        } else {
            match s[i] {
                '\n' => {
                    nl = true;
                    ws = true;
                }
                ' ' | '\t' | '\x0B' | '\x0C' => {
                    ws = true;
                }
                _ => {}
            }
            i += 1;
        }
        options.clear();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn tokenize(s: &str) -> Vec<PPToken> {
        let fv: elsa::FrozenVec<Box<[char]>> = Default::default();
        fv.push(s.chars().collect::<Vec<char>>().into_boxed_slice());
        let vec = filter_esc_nl_and_rep_comments(fv[0].iter().peekable());
        pp_tokenize(&vec, &fv[0], 0)
    }

    /// `span` and `hs` are ignored by `PPToken`'s `PartialEq`, so use dummies.
    pub fn tok(text: &str, kind: PPKind, nl: bool, ws: bool) -> PPToken {
        PPToken {
            text: text.to_string(),
            kind,
            nl,
            ws,
            span: Span { start: 0, end: 0, src: 0 },
            hs: HideSet::default(),
        }
    }

    /// Run the full phase-1..4 pipeline (splice, comments, tokenize,
    /// directives + macro expansion) over a source string.
    fn preprocess_str(s: &str) -> Vec<PPToken> {
        let sm = SourceManager::new();
        let iter = sm.add_source(
            s.chars().collect::<Vec<_>>().into_boxed_slice(),
            std::path::PathBuf::from("test.c"),
        );
        let content = filter_esc_nl_and_rep_comments(iter);
        let mut macros = directive::Macros::default();
        Lexer::new(&sm).preprocess(&content, &mut macros).unwrap()
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
        insta::assert_snapshot!(format!("{}\n\n{}", rendered.trim_start(), listing.join("\n")));
    }

    #[test]
    fn header_precedence() {
        const S: &str = r#"#include "1/a.c"
    #include <2/a.h>
    include <2/a.h>
    include "2/a.h""#;

        let result = tokenize(S);

        let expected = vec![
            tok("#", PPKind::Punct, true, true),
            tok("include", PPKind::Ident, false, false),
            tok("\"1/a.c\"", PPKind::Header, false, true),
            tok("#", PPKind::Punct, true, true),
            tok("include", PPKind::Ident, false, false),
            tok("<2/a.h>", PPKind::Header, false, true),
            tok("include", PPKind::Ident, true, true),
            tok("<", PPKind::Punct, false, true),
            tok("2", PPKind::PPNumber, false, false),
            tok("/", PPKind::Punct, false, false),
            tok("a", PPKind::Ident, false, false),
            tok(".", PPKind::Punct, false, false),
            tok("h", PPKind::Ident, false, false),
            tok(">", PPKind::Punct, false, false),
            tok("include", PPKind::Ident, true, true),
            tok("\"2/a.h\"", PPKind::StrLit, false, true),
        ];

        assert_eq!(result, expected, "{result:?} != {expected:?}");
    }

    #[test]
    fn pp_token() {
        const S: &str = r#"0x3<1/a.h>1e2
    #include <1/a.h>
    #define const.member@$
    "#;

        let result = tokenize(S);

        let expected = vec![
            tok("0x3", PPKind::PPNumber, true, true),
            tok("<", PPKind::Punct, false, false),
            tok("1", PPKind::PPNumber, false, false),
            tok("/", PPKind::Punct, false, false),
            tok("a", PPKind::Ident, false, false),
            tok(".", PPKind::Punct, false, false),
            tok("h", PPKind::Ident, false, false),
            tok(">", PPKind::Punct, false, false),
            tok("1e2", PPKind::PPNumber, false, false),
            tok("#", PPKind::Punct, true, true),
            tok("include", PPKind::Ident, false, false),
            tok("<1/a.h>", PPKind::Header, false, true),
            tok("#", PPKind::Punct, true, true),
            tok("define", PPKind::Ident, false, false),
            tok("const", PPKind::Ident, false, true),
            tok(".", PPKind::Punct, false, false),
            tok("member", PPKind::Ident, false, false),
            tok("@", PPKind::Other, false, false),
            tok("$", PPKind::Other, false, false),
        ];

        assert_eq!(result, expected, "{result:?} != {expected:?}");
    }

    #[test]
    fn pp_pluses() {
        const S: &str = r#"x+++++y"#;

        let result = tokenize(S);

        let expected = vec![
            tok("x", PPKind::Ident, true, true),
            tok("++", PPKind::Punct, false, false),
            tok("++", PPKind::Punct, false, false),
            tok("+", PPKind::Punct, false, false),
            tok("y", PPKind::Ident, false, false),
        ];

        assert_eq!(result, expected, "{result:?} != {expected:?}");
    }

    #[test]
    fn pp_ints() {
        const S: &str = r#"
0 1 42 1000000
1U 1u 1L 1l 1UL 1Lu 1LU 1uL 1LL 1ll 1ULL 1uLL 1LLu 1LLU 1llu 1llU
0 07 0777 010U 0123L 0123456701234567ULL
0x1 0X1 0xFF 0x1U 0x1u 0x1L 0x1UL 0x1LU 0x1LL 0x1ULL 0xDEADBEEFuLL 0Xabcdef
0b1 0B1 0b1010 0b1010U 0b1010UL 0b1010ULL 0b101010101
1wb 1uwb 1Uwb 1uWB 1WB 0x1Fwb 0b101uwb 123456789012345wb
1'000 1'000'000 0x1'FFFF 0b1010'1010 1'000U 1'000'000ULL 0xFF'FF'FF'FFu
1.0 1. .1 0.0 3.14159 1.0f 1.0F 1.0l 1.0L .5f 5.f 1e10 1E10 1e+10 1e-10 1.5e10 1.5e+10 1.5e-10 1.5e10f 1.5e-10L 1'000.5 1.000'001
0x1p0 0x1P0 0x1.8p3 0x1.8p+3 0x1.8p-3 0x1p3f 0x1.8p-3L 0x1FFp10 0x.1p4 0x1.p4
1.2.3 0x1.8 0x 0x1p 1e 1e+ 09 0128 1ULLULL 123abc 1.0.0f 0b 0b2 1wb2 1uwbu
"#;

        let result = tokenize(S);

        insta::assert_debug_snapshot!(result);
    }
}
