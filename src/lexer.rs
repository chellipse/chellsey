/// Does not currently handle [UAX #31](https://www.unicode.org/reports/tr31/)
use std::{
    fmt::Write,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
use constcat::concat_slices;
use elsa::{index_set::FrozenIndexSet, FrozenVec};
use strum_macros::EnumString;

mod dfa;
use dfa::*;

mod directive;
use directive::*;

// 6.4.1
#[derive(Debug, EnumString)]
#[allow(non_camel_case_types)]
enum Keyword {
    #[strum(serialize = "alignas", serialize = "_Alignas")]
    alignas,
    #[strum(serialize = "alignof", serialize = "_Alignof")]
    alignof,
    auto,
    #[strum(serialize = "bool", serialize = "_Bool")]
    bool,
    #[strum(serialize = "break")]
    _break,
    case,
    char,
    #[strum(serialize = "const")]
    _const,
    constexpr,
    #[strum(serialize = "continue")]
    _continue,
    default,
    #[strum(serialize = "do")]
    _do,
    double,
    #[strum(serialize = "else")]
    _else,
    #[strum(serialize = "enum")]
    _enum,
    #[strum(serialize = "extern")]
    _extern,
    #[strum(serialize = "false")]
    _false,
    float,
    #[strum(serialize = "for")]
    _for,
    goto,
    #[strum(serialize = "if")]
    _if,
    inline,
    int,
    long,
    nullptr,
    register,
    restrict,
    #[strum(serialize = "return")]
    _return,
    short,
    signed,
    sizeof,
    #[strum(serialize = "static")]
    _static,
    #[strum(serialize = "static_assert", serialize = "_Static_assert")]
    static_assert,
    #[strum(serialize = "struct")]
    _struct,
    switch,
    #[strum(serialize = "thread_local", serialize = "_Thread_local")]
    thread_local,
    #[strum(serialize = "true")]
    _true,
    typedef,
    #[strum(serialize = "typeof")]
    _typeof,
    typeof_unqual,
    union,
    unsigned,
    void,
    volatile,
    _while,
    _Atomic,
    _BitInt,
    _Complex,
    _Decimal128,
    _Decimal32,
    _Decimal64,
    _Generic,
    _Imaginary,
    _Noreturn,
}

#[derive(Debug)]
enum Token {
    Kw(Keyword),
    Identifier,
}

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

const INCLUDE: &[char] = &['i', 'n', 'c', 'l', 'u', 'd', 'e'];
const EMBED: &[char] = &['e', 'm', 'b', 'e', 'd'];
const DEFINE: &[char] = &['d', 'e', 'f', 'i', 'n', 'e'];
const VA_ARGS: &[char] = &['_', '_', 'V', 'A', '_', 'A', 'R', 'G', 'S', '_', '_'];
const VA_OPT: &[char] = &['_', '_', 'V', 'A', '_', 'O', 'P', 'T', '_', '_'];
const ELLIPSIS: &[char] = &['.', '.', '.'];

// preprocessing-tokens: 3..=6

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum PPKind {
    Header,
    Ident,
    PPNumber,
    CharConst,
    StrLit,
    Punct,
    UCN,
    Other,
}

#[derive(Clone, PartialEq)]
pub struct PPToken<'a> {
    text: &'a [char],
    kind: PPKind,
    nl: bool,
    ws: bool,
}

impl std::fmt::Debug for PPToken<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_char('{')?;
        for c in self.text.iter() {
            f.write_char(*c)?;
        }
        f.write_char('}')
    }
}

#[derive(Default)]
pub struct Lexer {
    char_contents: FrozenVec<Box<[char]>>,
    paths_opened: FrozenIndexSet<PathBuf>,
}

impl Lexer {
    pub fn new() -> Self {
        Default::default()
    }

    fn open(&self, path: impl AsRef<Path>) -> Result<&[char]> {
        let path = path.as_ref().to_path_buf();
        if self.paths_opened.get(&path).is_some() {
            // TODO: as long as we do serial depth first inclusion i think we
            // can just ignore repeated paths since they are already in the
            // file
            Err(anyhow!(
                "File has already been opened! No include recursion!!!"
            ))
        } else {
            let mut f = File::open(path)?;
            let mut buf = String::new();
            f.read_to_string(&mut buf)?;

            let mut vec = Vec::new();
            // phase 1
            let mut iter = buf.chars().peekable();

            // phase 2
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
                if let Some(l) = dfa_comment(&vec[i..]).ok() {
                    // println!(
                    //     "REMOVAING COMMENT: {:?}",
                    //     &vec[i..i + l].iter().collect::<String>()
                    // );
                    vec.splice(i..i + l, std::iter::once(' '));
                } else {
                    i += 1;
                }
            }

            self.char_contents.push(vec.into_boxed_slice());

            Ok(self.char_contents.last().unwrap())
        }
    }

    pub fn lex(self, path: impl AsRef<Path>) -> Result<()> {
        let content = self.open(path)?;

        let tokens = self.preprocess(content)?;

        println!("{:?}", tokens);

        todo!("Phase 5");
    }

    fn preprocess<'a>(&'a self, content: &'a [char]) -> Result<Vec<PPToken<'a>>> {
        let mut tokens = pp_tokenize(&content);

        let mut i = 0;

        while i < tokens.len() {
            if let PPToken { text: &['#'], kind: PPKind::Punct, nl: true, .. } = tokens[i] {
                // TODO: make last token {#} an error earlier
                match &tokens[i + 1..] {
                    // TODO: make include w/o header-name error
                    [PPToken { text: INCLUDE, kind: PPKind::Ident, .. }, PPToken { text: path, kind: PPKind::Header, .. }, ..] =>
                    {
                        let path = &path[1..path.len() - 1];
                        let path = path.iter().collect::<String>();
                        let content = self.open(path)?;
                        let more_tokens = self.preprocess(content)?;
                        tokens.splice(i..i + 3, more_tokens.into_iter());
                    }
                    // TODO: make define w/o identifier error
                    [PPToken { text: DEFINE, kind: PPKind::Ident, .. }, PPToken { text: key, kind: PPKind::Ident, .. }, ..] =>
                    {
                        // TODO make \n#\n error
                        let eol = tokens[i + 3..].iter().position(|x| x.nl).unwrap();
                        let tokens = &tokens[i + 3..i + eol];

                        if let PPToken { text, kind, nl, ws } = tokens[0] {}

                        todo!()
                    }
                    x => {
                        todo!("TODO: {:?}", x);
                        // i += 1;
                    }
                };
            } else {
                i += 1;
            }
        }

        Ok(tokens)
    }
}

pub fn pp_tokenize(s: &[char]) -> Vec<PPToken<'_>> {
    let mut i = 0;
    let mut options: Vec<(usize, PPKind)> = Vec::new();
    let mut result: Vec<PPToken> = Vec::new();
    let mut nl = true;
    let mut ws = true;

    while i < s.len() {
        let slice = &s[i..];

        // TODO: need more line state here
        if let [.., PPToken { text: ['#'], kind: PPKind::Punct, nl: true, .. }, PPToken { text: INCLUDE | EMBED, kind: PPKind::Ident, .. }] =
            result.as_slice()
        {
            if let Some(l) = dfa_header_name(slice).ok() {
                options.push((l, PPKind::Header));
            }
        }

        if let Some(l) = dfa_identifier(slice).ok() {
            options.push((l, PPKind::Ident));
        }

        if let Some(l) = dfa_pp_number(slice).ok() {
            options.push((l, PPKind::PPNumber));
        }

        if let Some(l) = dfa_character_constant(slice).ok() {
            options.push((l, PPKind::CharConst));
        }

        if let Some(l) = dfa_string_literal(slice).ok() {
            options.push((l, PPKind::StrLit));
        }

        if let Some(l) = dfa_punctuator(slice).ok() {
            options.push((l, PPKind::Punct));
        }

        if let Some(l) = dfa_ucn(slice).ok() {
            options.push((l, PPKind::UCN));
        }

        if let Some(l) = dfa_other(slice).ok() {
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
            let text = &slice[..len];
            i += len;
            result.push(PPToken { text, kind, nl, ws });
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

    #[test]
    fn header_precedence() {
        const S: &str = r#"#include "1/a.c"
#include <2/a.h>
include <2/a.h>
include "2/a.h""#;

        let s = S.chars().collect::<Vec<char>>();
        let result = pp_tokenize(&s);

        let expected = vec![
            PPToken { text: &s[0..1], kind: PPKind::Punct, nl: true, ws: true },
            PPToken { text: &s[1..8], kind: PPKind::Ident, nl: false, ws: false },
            PPToken { text: &s[9..16], kind: PPKind::Header, nl: false, ws: true },
            PPToken { text: &s[17..18], kind: PPKind::Punct, nl: true, ws: true },
            PPToken { text: &s[18..25], kind: PPKind::Ident, nl: false, ws: false },
            PPToken { text: &s[26..33], kind: PPKind::Header, nl: false, ws: true },
            PPToken { text: &s[34..41], kind: PPKind::Ident, nl: true, ws: true },
            PPToken { text: &s[42..43], kind: PPKind::Punct, nl: false, ws: true },
            PPToken { text: &s[43..44], kind: PPKind::PPNumber, nl: false, ws: false },
            PPToken { text: &s[44..45], kind: PPKind::Punct, nl: false, ws: false },
            PPToken { text: &s[45..46], kind: PPKind::Ident, nl: false, ws: false },
            PPToken { text: &s[46..47], kind: PPKind::Punct, nl: false, ws: false },
            PPToken { text: &s[47..48], kind: PPKind::Ident, nl: false, ws: false },
            PPToken { text: &s[48..49], kind: PPKind::Punct, nl: false, ws: false },
            PPToken { text: &s[50..57], kind: PPKind::Ident, nl: true, ws: true },
            PPToken { text: &s[58..65], kind: PPKind::StrLit, nl: false, ws: true },
        ];

        assert_eq!(result, expected, "{result:?} != {expected:?}");
    }

    #[test]
    fn pp_token() {
        const S: &str = r#"0x3<1/a.h>1e2
#include <1/a.h>
#define const.member@$
"#;

        let s = S.chars().collect::<Vec<char>>();
        let result = pp_tokenize(&s);

        let expected = vec![
            PPToken { text: &s[0..3], kind: PPKind::PPNumber, nl: true, ws: true },
            PPToken { text: &s[3..4], kind: PPKind::Punct, nl: false, ws: false },
            PPToken { text: &s[4..5], kind: PPKind::PPNumber, nl: false, ws: false },
            PPToken { text: &s[5..6], kind: PPKind::Punct, nl: false, ws: false },
            PPToken { text: &s[6..7], kind: PPKind::Ident, nl: false, ws: false },
            PPToken { text: &s[7..8], kind: PPKind::Punct, nl: false, ws: false },
            PPToken { text: &s[8..9], kind: PPKind::Ident, nl: false, ws: false },
            PPToken { text: &s[9..10], kind: PPKind::Punct, nl: false, ws: false },
            PPToken { text: &s[10..13], kind: PPKind::PPNumber, nl: false, ws: false },
            PPToken { text: &s[14..15], kind: PPKind::Punct, nl: true, ws: true },
            PPToken { text: &s[15..22], kind: PPKind::Ident, nl: false, ws: false },
            PPToken { text: &s[23..30], kind: PPKind::Header, nl: false, ws: true },
            PPToken { text: &s[31..32], kind: PPKind::Punct, nl: true, ws: true },
            PPToken { text: &s[32..38], kind: PPKind::Ident, nl: false, ws: false },
            PPToken { text: &s[39..44], kind: PPKind::Ident, nl: false, ws: true },
            PPToken { text: &s[44..45], kind: PPKind::Punct, nl: false, ws: false },
            PPToken { text: &s[45..51], kind: PPKind::Ident, nl: false, ws: false },
            PPToken { text: &s[51..52], kind: PPKind::Other, nl: false, ws: false },
            PPToken { text: &s[52..53], kind: PPKind::Other, nl: false, ws: false },
        ];

        assert_eq!(result, expected, "{result:?} != {expected:?}");
    }

    #[test]
    fn pp_pluses() {
        const S: &str = r#"x+++++y"#;

        let s = S.chars().collect::<Vec<char>>();
        let result = pp_tokenize(&s);

        let expected = vec![
            PPToken { text: &s[0..1], kind: PPKind::Ident, nl: true, ws: true },
            PPToken { text: &s[1..3], kind: PPKind::Punct, nl: false, ws: false },
            PPToken { text: &s[3..5], kind: PPKind::Punct, nl: false, ws: false },
            PPToken { text: &s[5..6], kind: PPKind::Punct, nl: false, ws: false },
            PPToken { text: &s[6..7], kind: PPKind::Ident, nl: false, ws: false },
        ];

        assert_eq!(result, expected, "{result:?} != {expected:?}");
    }
}
