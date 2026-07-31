/// Does not currently handle [UAX #31](https://www.unicode.org/reports/tr31/)
use std::{iter::Peekable, path::Path};

use anyhow::{Result, anyhow};
use constcat::concat_slices;

mod dfa;
mod directive;
mod preprocess;
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

    pub fn lex(self, path: impl AsRef<Path>) -> Result<Vec<Token>> {
        // translation phases 1–4 live in the `Preprocessor`
        let tokens = preprocess::Preprocessor::new(self.sm).process(path)?;
        println!("PPTokens: {:?}", &tokens);

        promote_all(tokens)
    }
}

/// Phases 5–7: promote each preprocessing token into a real token, and
/// (phase 6) concatenate adjacent string literals into one token whose span
/// covers the whole run.
fn promote_all(tokens: Vec<PPToken>) -> Result<Vec<Token>> {
    let mut result: Vec<Token> = Vec::with_capacity(tokens.len());

    for pp in tokens {
        // `promote` yields a span-carrying `diagnostic::Error`; `?` folds it
        // into `anyhow`, and `main` resolves it against `SOURCES`.
        let token = pp.promote()?;

        if let TokenKind::StrLit { value, .. } = &token.kind
            && let Some(Token { kind: TokenKind::StrLit { value: prev, .. }, span }) =
                result.last_mut()
        {
            prev.extend_from_slice(value);
            *span = span.union(&token.span);
        } else {
            result.push(token);
        }
    }

    Ok(result)
}

// TODO: make this not run inside string literals
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

    fn lex_str(s: &str) -> Vec<Token> {
        promote_all(tokenize(s)).unwrap()
    }

    fn kinds(tokens: &[Token]) -> Vec<&TokenKind> {
        tokens.iter().map(|t| &t.kind).collect()
    }

    #[test]
    fn promote_ints() {
        let toks = lex_str(
            "0 42 1000000 1U 1l 1UL 1LL 1ull 1LLu 0777 010U 0x1F 0Xff 0xDEADBEEFuLL \
             0b1010 0B1 1'000'000 0xFF'FFu 18446744073709551615",
        );
        insta::assert_debug_snapshot!(kinds(&toks));
    }

    #[test]
    fn promote_floats() {
        let toks = lex_str(
            "1.0 .5f 5.f 3.14159 1e10 1E-10 1.5e+10L 1'000.5 \
             0x1p0 0x1P0 0x1.8p3 0x1.8p-3 0x.1p4 0x1.p4 0x1p3f 0x1.8p-3L",
        );
        insta::assert_debug_snapshot!(kinds(&toks));
    }

    #[test]
    fn promote_chars() {
        let toks = lex_str(r#"'a' '\n' '\0' '\x41' '\'' '\102'"#);
        insta::assert_debug_snapshot!(kinds(&toks));
    }

    #[test]
    fn promote_strings() {
        let toks = lex_str(r#""ab" "a\tb" "\x41\102" "é" "\0777""#);
        insta::assert_debug_snapshot!(kinds(&toks));
    }

    #[test]
    fn string_concat() {
        // phase 6: adjacent literals merge; anything else breaks the run
        let toks = lex_str(r#""foo" "bar" 1 "a" "b" "c""#);
        assert_eq!(toks.len(), 3);
        assert!(matches!(&toks[0].kind, TokenKind::StrLit { value, .. } if value == b"foobar"));
        assert!(matches!(
            &toks[1].kind,
            TokenKind::IntConst { value: 1, .. }
        ));
        assert!(matches!(&toks[2].kind, TokenKind::StrLit { value, .. } if value == b"abc"));
    }

    /// Independently derives every order/case combination 6.4.4.1 allows, so
    /// a typo in `parse_int_suf`'s spelling table can't hide in an untested
    /// arm.
    #[test]
    fn int_suffix_combinations() {
        for u in ["u", "U"] {
            for (l, len) in [
                ("l", IntLen::Long),
                ("L", IntLen::Long),
                ("ll", IntLen::LongLong),
                ("LL", IntLen::LongLong),
            ] {
                for s in [format!("1{u}{l}"), format!("1{l}{u}")] {
                    let toks = lex_str(&s);
                    assert!(
                        matches!(
                            &toks[0].kind,
                            TokenKind::IntConst { suf: IntSuf { unsigned: true, len: l2 }, .. }
                                if *l2 == len
                        ),
                        "`{s}` should be unsigned {len:?}"
                    );
                }
            }
            // wb in every valid order/case: still rejected (TBD), not junk
            for wb in ["wb", "WB"] {
                for s in [format!("1{u}{wb}"), format!("1{wb}{u}")] {
                    assert!(tokenize(&s)[0].promote().is_err(), "`{s}` is TBD");
                }
            }
        }
    }

    #[test]
    fn promote_rejects_invalid() {
        for src in [
            "09",
            "0128",
            "0x",
            "0b",
            "0b2",
            "1e",
            "1e+",
            "123abc",
            "1ULLULL",
            "1lL",
            "1.0.0f",
            "1.2.3",
            "0x1.8",
            "0x1p",
            "1wb2",
            "1uwbu",
            "18446744073709551616",
            "'ab'",
            r"'\777'",
            // valid C23, but `_BitInt` is TBD — must error, not demote to int
            "1wb",
            "1uwb",
            "0x1Fwb",
        ] {
            let pp = tokenize(src);
            assert_eq!(pp.len(), 1, "{src} should be one pp-token");
            assert!(pp[0].promote().is_err(), "`{src}` should be rejected");
        }
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
