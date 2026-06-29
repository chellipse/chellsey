/// Does not currently handle [UAX #31](https://www.unicode.org/reports/tr31/)
use std::{
    fmt::Write,
    fs::File,
    io::Read,
    iter::Peekable,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow};
use constcat::concat_slices;
use strum_macros::EnumString;

mod dfa;
mod diagnostic;
mod directive;

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

const INCLUDE: &str = "include";
const EMBED: &str = "embed";
const DEFINE: &str = "define";
const VA_ARGS: &str = "__VA_ARGS__";
const VA_OPT: &str = "__VA_OPT__";
const ELLIPSIS: &str = "...";
const LPAREN: &str = "(";
const RPAREN: &str = ")";
const COMMA: &str = ",";
const SPACE: &char = &' ';

#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    start: usize,
    end: usize,
    src: usize,
}

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

/// `PartialEq` impl *ignores* self.span- for testing
#[derive(Clone)]
pub struct PPToken {
    text: String,
    kind: PPKind,
    nl: bool,
    ws: bool,
    span: Span,
}

impl std::fmt::Debug for PPToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{{}:{:?}}}", self.text, self.kind)
    }
}

impl PartialEq for PPToken {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
            && self.kind == other.kind
            && self.nl == other.nl
            && self.ws == other.ws
    }
}

#[derive(Default)]
pub struct Lexer {
    char_contents: elsa::FrozenVec<Box<[char]>>,
    paths_opened: elsa::index_set::FrozenIndexSet<PathBuf>,
}

impl Lexer {
    pub fn new() -> Self {
        Default::default()
    }

    fn add_source(
        &self,
        content: Box<[char]>,
        path: PathBuf,
    ) -> Peekable<impl Iterator<Item = &char>> {
        self.char_contents.push(content);
        self.paths_opened.insert(path);
        self.char_contents.last().unwrap().iter().peekable()
    }

    fn open(&self, path: impl AsRef<Path>) -> Result<Option<Vec<&char>>> {
        let path = path.as_ref().to_path_buf();
        if self.paths_opened.get(&path).is_none() {
            let mut f = File::open(&path)?;
            let mut buf = String::new();
            f.read_to_string(&mut buf)?;

            // phase 1
            let iter = self.add_source(buf.chars().collect::<Vec<_>>().into_boxed_slice(), path);

            // phase 2 and partial phase 3, replacing comments with spaces
            let vec = filter_esc_nl_and_rep_comments(iter);

            return Ok(Some(vec));
        }

        Ok(None)
    }

    pub fn lex(self, path: impl AsRef<Path>) -> Result<()> {
        // can't be None cause that's just if we've opened it before...
        let content = self.open(path)?.unwrap();

        let tokens = self.preprocess(&content)?;

        println!("{:?}", tokens);

        todo!("Phase 5");
    }

    fn preprocess(&self, content: &[&char]) -> Result<Vec<PPToken>> {
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
                            let more_tokens = self.preprocess(&content)?;
                            tokens.splice(i..i + 3, more_tokens.into_iter());
                        };
                    }
                    // TODO: make define w/o identifier error
                    [
                        PPToken { text, kind: PPKind::Ident, .. },
                        PPToken { text: key, kind: PPKind::Ident, .. },
                        ..,
                    ] if text.as_str() == "define" => {
                        // TODO make \n#\n error
                        let eol = tokens[i + 3..].iter().position(|x| x.nl).unwrap();
                        let tokens = &tokens[i + 3..i + eol];

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

    fn pp_tokenize(&self, s: &[&char]) -> Vec<PPToken> {
        let src = self.char_contents.len() - 1;
        let content = &self.char_contents[src];
        pp_tokenize(s, content, src)
    }

    fn diagnostic(&self, span: &Span, msg: Option<String>) {
        let path = &self.paths_opened[span.src];
        let slice = &self.char_contents[span.src];

        diagnostic::show(path, slice, span, msg)
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
            && (it == "include" || it == "embed")
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
            result.push(PPToken { text, kind, nl, ws, span });
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

    /// `span` is ignored by `PPToken`'s `PartialEq`, so use a dummy one.
    pub fn tok(text: &str, kind: PPKind, nl: bool, ws: bool) -> PPToken {
        PPToken {
            text: text.to_string(),
            kind,
            nl,
            ws,
            span: Span { start: 0, end: 0, src: 0 },
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
}
