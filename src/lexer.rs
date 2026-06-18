use std::fmt::Write;

/// Does not currently handle [UAX #31](https://www.unicode.org/reports/tr31/)
use constcat::concat_slices;
use strum_macros::EnumString;

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

// preprocessing-tokens: 3..=6

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum PPKind {
    HeaderName,
    Identifier,
    PPNumber,
    CharacterConstant,
    StringLiteral,
    Punctuator,
    UCN,
    Other,
}

#[derive(PartialEq)]
pub struct PPToken<'a> {
    text: &'a [char],
    kind: PPKind,
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

pub fn lex(s: &str) {
    let mut vec = Vec::new();
    let mut iter = s.chars().peekable();

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

    let pp_tokens = pp_tokenize(&vec);

    println!("{:?}", pp_tokens);
}

pub fn pp_tokenize(s: &[char]) -> Vec<PPToken<'_>> {
    let mut i = 0;
    let mut options: Vec<(usize, PPKind)> = Vec::new();
    let mut result = Vec::new();

    while i < s.len() {
        let slice = &s[i..];
        // TODO: need more line state here
        if let Some(l) = dfa_header_name(slice).ok() {
            if let Some(&['i', 'n', 'c', 'l', 'u', 'd', 'e'] | &['e', 'm', 'b', 'e', 'd']) =
                result.last().map(|x: &PPToken| x.text)
            {
                options.push((l, PPKind::HeaderName));
            }
        }

        if let Some(l) = dfa_identifier(slice).ok() {
            options.push((l, PPKind::Identifier));
        }

        if let Some(l) = dfa_pp_number(slice).ok() {
            options.push((l, PPKind::PPNumber));
        }

        if let Some(l) = dfa_character_constant(slice).ok() {
            options.push((l, PPKind::CharacterConstant));
        }

        if let Some(l) = dfa_string_literal(slice).ok() {
            options.push((l, PPKind::StringLiteral));
        }

        if let Some(l) = dfa_punctuator(slice).ok() {
            options.push((l, PPKind::Punctuator));
        }

        if let Some(l) = dfa_ucn(slice).ok() {
            options.push((l, PPKind::UCN));
        }

        if let Some(l) = dfa_other(slice).ok() {
            options.push((l, PPKind::Other));
        }

        // maximal munch, no token precedence aside from
        // header-name / string-literal as described in 6.4 paragraph 4
        options.sort_by(|a, b| b.0.cmp(&a.0));
        if let Some(last) = options.get(0) {
            let len = last.0;
            let kind = last.1;
            let text = &slice[..len];
            i += len;
            result.push(PPToken { text, kind });
            println!("{:?} {:?}", result.last().unwrap(), kind);
        } else {
            i += 1;
        }
        options.clear();
    }

    result
}

fn dfa_header_name(s: &[char]) -> Result<usize, String> {
    #[derive(Debug)]
    enum St {
        Ent,
        HCont,
        HTerm,
        QCont,
        QTerm,
    }
    let mut st = St::Ent;
    let mut result = None;
    let mut last = None;

    const INV_H: &[char] = &['\n', '>'];
    const INV_Q: &[char] = &['\n', '"'];

    for (i, c) in s.iter().enumerate() {
        st = match (&st, c) {
            (St::Ent, '<') => St::HCont,
            (St::HCont, x) if !INV_H.contains(x) => St::HCont,
            (St::HCont, '>') => St::HTerm,
            (St::Ent, '"') => St::QCont,
            (St::QCont, x) if !INV_Q.contains(x) => St::QCont,
            (St::QCont, '"') => St::QTerm,
            _ => break,
        };

        if matches!(st, St::HTerm | St::QTerm) {
            result = Some(i)
        }

        last = Some(i);
    }

    result.map(|x| x + 1).ok_or(format!("{st:?}:{last:?}"))
}

fn dfa_pp_number(s: &[char]) -> Result<usize, String> {
    #[derive(Debug)]
    enum St {
        Ent,
        Period,
        Cont,
        Apostrophe,
        SignLetter,
    }
    let mut st = St::Ent;
    let mut result = None;
    let mut last = None;

    const SIGN_LETTERS: &[char] = &['e', 'E', 'p', 'P'];
    const SIGNS: &[char] = &['-', '+'];

    for (i, c) in s.iter().enumerate() {
        st = match (&st, c) {
            (St::Ent, x) if DIGIT.contains(x) => St::Cont,
            (St::Ent, '.') => St::Period,
            (St::Period, x) if DIGIT.contains(x) => St::Cont,
            (St::Cont, x) if IDENT_CONT.contains(x) => St::Cont,
            (St::Cont, '\'') => St::Apostrophe,
            (St::Apostrophe, x) if DIGIT_AND_NON_DIGIT.contains(x) => St::Cont,
            (St::Cont, x) if SIGN_LETTERS.contains(x) => St::SignLetter,
            (St::SignLetter, x) if SIGNS.contains(x) => St::Cont,
            (St::Cont, '.') => St::Cont,
            _ => break,
        };

        if matches!(st, St::Cont) {
            result = Some(i)
        }

        last = Some(i);
    }

    result.map(|x| x + 1).ok_or(format!("{st:?}:{last:?}"))
}

fn dfa_character_constant(s: &[char]) -> Result<usize, String> {
    #[derive(Debug)]
    enum St {
        Ent,
        PreU,
        Prefix,
        Start,
        Cont,
        Esc,
        Octal1,
        Octal2,
        HexEnt,
        Hex,
        Term,
        UCNLower0,
        UCNLower1,
        UCNLower2,
        UCNLower3,
        UCNUpper0,
        UCNUpper1,
        UCNUpper2,
        UCNUpper3,
        UCNUpper4,
        UCNUpper5,
        UCNUpper6,
        UCNUpper7,
    }
    let mut st = St::Ent;
    let mut result = None;
    let mut last = None;

    for (i, c) in s.iter().enumerate() {
        st = match (&st, c) {
            (St::Ent, 'u') => St::PreU,
            (St::PreU, '8') => St::Prefix,
            (St::Ent, 'U' | 'L') => St::Prefix,
            (St::Ent | St::PreU | St::Prefix, '\'') => St::Start,
            (St::Cont | St::Start, '\\') => St::Esc,
            (St::Esc, x) if SIMPLE_ESCAPES.contains(x) => St::Cont,
            (St::Esc, x) if OCTAL.contains(x) => St::Octal1,
            (St::Octal1, x) if OCTAL.contains(x) => St::Octal2,
            (St::Octal2, x) if OCTAL.contains(x) => St::Cont,
            (St::Esc, 'x') => St::HexEnt,
            (St::HexEnt, x) if HEX.contains(x) => St::Hex,
            (St::Hex, x) if HEX.contains(x) => St::Hex,
            (St::Esc, 'u') => St::UCNLower0,
            (St::UCNLower0, x) if HEX.contains(x) => St::UCNLower1,
            (St::UCNLower1, x) if HEX.contains(x) => St::UCNLower2,
            (St::UCNLower2, x) if HEX.contains(x) => St::UCNLower3,
            (St::UCNLower3, x) if HEX.contains(x) => St::Cont,
            (St::Esc, 'U') => St::UCNUpper0,
            (St::UCNUpper0, x) if HEX.contains(x) => St::UCNUpper1,
            (St::UCNUpper1, x) if HEX.contains(x) => St::UCNUpper2,
            (St::UCNUpper2, x) if HEX.contains(x) => St::UCNUpper3,
            (St::UCNUpper3, x) if HEX.contains(x) => St::UCNUpper4,
            (St::UCNUpper4, x) if HEX.contains(x) => St::UCNUpper5,
            (St::UCNUpper5, x) if HEX.contains(x) => St::UCNUpper6,
            (St::UCNUpper6, x) if HEX.contains(x) => St::UCNUpper7,
            (St::UCNUpper7, x) if HEX.contains(x) => St::Cont,
            (St::Cont | St::Octal1 | St::Octal2 | St::Hex, '\'') => St::Term,
            (St::Cont | St::Start | St::Octal1 | St::Octal2 | St::Hex, x)
                if !['\'', '\\', '\n'].contains(x) =>
            {
                St::Cont
            }
            _ => break,
        };

        if matches!(st, St::Term) {
            result = Some(i)
        }

        last = Some(i);
    }

    result.map(|x| x + 1).ok_or(format!("{st:?}:{last:?}"))
}

fn dfa_ucn(s: &[char]) -> Result<usize, String> {
    #[derive(Debug)]
    enum St {
        Ent,
        Esc,
        UCNLower0,
        UCNLower1,
        UCNLower2,
        UCNLower3,
        UCNUpper0,
        UCNUpper1,
        UCNUpper2,
        UCNUpper3,
        UCNUpper4,
        UCNUpper5,
        UCNUpper6,
        UCNUpper7,
        Term,
    }
    let mut st = St::Ent;
    let mut result = None;
    let mut last = None;

    for (i, c) in s.iter().enumerate() {
        st = match (&st, &c) {
            (St::Ent, '\\') => St::Esc,
            (St::Esc, 'u') => St::UCNLower0,
            (St::UCNLower0, x) if HEX.contains(x) => St::UCNLower1,
            (St::UCNLower1, x) if HEX.contains(x) => St::UCNLower2,
            (St::UCNLower2, x) if HEX.contains(x) => St::UCNLower3,
            (St::UCNLower3, x) if HEX.contains(x) => St::Term,
            (St::Esc, 'U') => St::UCNUpper0,
            (St::UCNUpper0, x) if HEX.contains(x) => St::UCNUpper1,
            (St::UCNUpper1, x) if HEX.contains(x) => St::UCNUpper2,
            (St::UCNUpper2, x) if HEX.contains(x) => St::UCNUpper3,
            (St::UCNUpper3, x) if HEX.contains(x) => St::UCNUpper4,
            (St::UCNUpper4, x) if HEX.contains(x) => St::UCNUpper5,
            (St::UCNUpper5, x) if HEX.contains(x) => St::UCNUpper6,
            (St::UCNUpper6, x) if HEX.contains(x) => St::UCNUpper7,
            (St::UCNUpper7, x) if HEX.contains(x) => St::Term,
            _ => break,
        };

        if matches!(st, St::Term) {
            result = Some(i)
        }

        last = Some(i);
    }

    result.map(|x| x + 1).ok_or(format!("{st:?}:{last:?}"))
}

fn dfa_other(s: &[char]) -> Result<usize, String> {
    match s.get(0) {
        Some(' ' | '\n') => Err("Invalid".to_string()),
        Some(_) => Ok(1),
        None => Err("EOF".to_string()),
    }
}

// tokens: 7..=8

fn dfa_identifier(s: &[char]) -> Result<usize, String> {
    #[derive(Debug)]
    enum St {
        Ent,
        Cont,
    }
    let mut st = St::Ent;
    let mut result = None;
    let mut last = None;

    for (i, c) in s.iter().enumerate() {
        st = match (&st, c) {
            (St::Ent, x) if NON_DIGIT.contains(x) => St::Cont,
            (St::Cont, x) if DIGIT_AND_NON_DIGIT.contains(x) => St::Cont,
            _ => break,
        };

        if matches!(st, St::Cont) {
            result = Some(i)
        }

        last = Some(i)
    }

    result.map(|x| x + 1).ok_or(format!("{st:?}:{last:?}"))
}

fn dfa_constant(_: &[char]) -> Result<usize, String> {
    Result::Err("unimplemented".to_string())
}

fn dfa_string_literal(s: &[char]) -> Result<usize, String> {
    #[derive(Debug)]
    enum St {
        Ent,
        PreU,
        Prefix,
        Cont,
        Esc,
        Octal1,
        Octal2,
        HexEnt,
        Hex,
        Term,
        UCNLower0,
        UCNLower1,
        UCNLower2,
        UCNLower3,
        UCNUpper0,
        UCNUpper1,
        UCNUpper2,
        UCNUpper3,
        UCNUpper4,
        UCNUpper5,
        UCNUpper6,
        UCNUpper7,
    }
    let mut st = St::Ent;
    let mut result = None;
    let mut last = None;

    for (i, c) in s.iter().enumerate() {
        st = match (&st, c) {
            (St::Ent, 'u') => St::PreU,
            (St::PreU, '8') => St::Prefix,
            (St::Ent, 'U' | 'L') => St::Prefix,
            (St::Ent | St::PreU | St::Prefix, '"') => St::Cont,
            (St::Cont, '\\') => St::Esc,
            (St::Esc, x) if SIMPLE_ESCAPES.contains(x) => St::Cont,
            (St::Esc, x) if OCTAL.contains(x) => St::Octal1,
            (St::Octal1, x) if OCTAL.contains(x) => St::Octal2,
            (St::Octal2, x) if OCTAL.contains(x) => St::Cont,
            (St::Esc, 'x') => St::HexEnt,
            (St::HexEnt, x) if HEX.contains(x) => St::Hex,
            (St::Hex, x) if HEX.contains(x) => St::Hex,
            (St::Esc, 'u') => St::UCNLower0,
            (St::UCNLower0, x) if HEX.contains(x) => St::UCNLower1,
            (St::UCNLower1, x) if HEX.contains(x) => St::UCNLower2,
            (St::UCNLower2, x) if HEX.contains(x) => St::UCNLower3,
            (St::UCNLower3, x) if HEX.contains(x) => St::Cont,
            (St::Esc, 'U') => St::UCNUpper0,
            (St::UCNUpper0, x) if HEX.contains(x) => St::UCNUpper1,
            (St::UCNUpper1, x) if HEX.contains(x) => St::UCNUpper2,
            (St::UCNUpper2, x) if HEX.contains(x) => St::UCNUpper3,
            (St::UCNUpper3, x) if HEX.contains(x) => St::UCNUpper4,
            (St::UCNUpper4, x) if HEX.contains(x) => St::UCNUpper5,
            (St::UCNUpper5, x) if HEX.contains(x) => St::UCNUpper6,
            (St::UCNUpper6, x) if HEX.contains(x) => St::UCNUpper7,
            (St::UCNUpper7, x) if HEX.contains(x) => St::Cont,
            (St::Cont | St::Octal1 | St::Octal2 | St::Hex, '"') => St::Term,
            (St::Cont | St::Octal1 | St::Octal2 | St::Hex, x) if !['"', '\\', '\n'].contains(x) => {
                St::Cont
            }
            _ => break,
        };

        if matches!(st, St::Term) {
            result = Some(i)
        }

        last = Some(i);
    }

    result.map(|x| x + 1).ok_or(format!("{st:?}:{last:?}"))
}

fn dfa_punctuator(s: &[char]) -> Result<usize, String> {
    #[derive(Debug)]
    enum St {
        Ent,
        Plus,
        Minus,
        Mul,
        Div,
        Xor,
        And,
        Or,
        Not,
        Eq,
        Period0,
        Period1,
        AngleL0,
        AngleL1,
        AngleR0,
        AngleR1,
        Colon,
        Pound,
        Percent0,
        Percent1,
        Percent2,
        Term,
    }
    let mut st = St::Ent;
    let mut result = None;
    let mut last = None;

    for (i, c) in s.iter().enumerate() {
        st = match (&st, c) {
            (St::Ent, '[' | ']' | '(' | ')' | '{' | '}' | ',' | ';' | '?' | '~') => St::Term,
            (St::Ent, '+') => St::Plus,
            (St::Plus, '+' | '=') => St::Term,
            (St::Ent, '-') => St::Minus,
            (St::Minus, '-' | '=' | '>') => St::Term,
            (St::Ent, '*') => St::Mul,
            (St::Mul, '*' | '=') => St::Term,
            (St::Ent, '/') => St::Div,
            (St::Div, '/' | '=') => St::Term,
            (St::Ent, '^') => St::Xor,
            (St::Xor, '^' | '=') => St::Term,
            (St::Ent, '&') => St::And,
            (St::And, '&' | '=') => St::Term,
            (St::Ent, '|') => St::Or,
            (St::Or, '|' | '=') => St::Term,
            (St::Ent, '!') => St::Not,
            (St::Not, '!' | '=') => St::Term,
            (St::Ent, '=') => St::Eq,
            (St::Eq, '=') => St::Term,
            (St::Ent, '.') => St::Period0,
            (St::Period0, '.') => St::Period1,
            (St::Period1, '.') => St::Term,
            (St::Ent, '<') => St::AngleL0,
            (St::AngleL0, ':' | '=' | '%') => St::Term,
            (St::AngleL0, '<') => St::AngleL1,
            (St::AngleL1, '=') => St::Term,
            (St::Ent, '>') => St::AngleR0,
            (St::AngleR0, '=') => St::Term,
            (St::AngleR0, '>') => St::AngleR1,
            (St::AngleR1, '=') => St::Term,
            (St::Ent, ':') => St::Colon,
            (St::Colon, ':' | '>') => St::Term,
            (St::Ent, '#') => St::Pound,
            (St::Pound, '#') => St::Term,
            (St::Ent, '%') => St::Percent0,
            (St::Percent0, '%' | '=' | '>') => St::Term,
            (St::Percent0, ':') => St::Percent1,
            (St::Percent1, '%') => St::Percent2,
            (St::Percent2, ':') => St::Term,
            _ => break,
        };

        if !matches!(st, St::Period1 | St::Percent2) {
            result = Some(i)
        }

        last = Some(i)
    }

    result.map(|x| x + 1).ok_or(format!("{st:?}:{last:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_eq_wrapper<T: Fn(&[char]) -> Result<usize, String>>(
        func: T,
    ) -> impl Fn(&str, Option<&str>) {
        move |input: &str, expected: Option<&str>| {
            let input_chars = input.chars().collect::<Vec<_>>();

            let result = func(&input_chars)
                .ok()
                .map(|x| input_chars[..x].iter().collect::<String>());

            assert_eq!(
                expected,
                result.as_ref().map(|x| x.as_str()),
                "f({input:?}) -> {result:?} != f({input:?}) -> {expected:?}",
            )
        }
    }

    #[test]
    fn string_literal() {
        let assert_eq = assert_eq_wrapper(dfa_string_literal);

        // --- basic s-char ---
        assert_eq("\"a\"", Some("\"a\""));
        assert_eq("\"abc\"", Some("\"abc\""));
        assert_eq("\" \"", Some("\" \"")); // space is a valid s-char
        assert_eq("\"'\"", Some("\"'\"")); // a single-quote needs no escape

        // --- s-char-sequence is OPTIONAL: the empty string is valid ---
        assert_eq("\"\"", Some("\"\""));

        // --- longest-match stops at the first unescaped closing quote ---
        assert_eq("\"x\" + \"y\"", Some("\"x\""));
        assert_eq("\"a\"\"b\"", Some("\"a\""));
        assert_eq("\"a\"; rest", Some("\"a\""));

        // --- encoding-prefix: u8 | u | U | L  (incl. empty body) ---
        assert_eq("u\"a\"", Some("u\"a\""));
        assert_eq("U\"a\"", Some("U\"a\""));
        assert_eq("L\"a\"", Some("L\"a\""));
        assert_eq("u8\"a\"", Some("u8\"a\""));
        assert_eq("u\"\"", Some("u\"\""));
        assert_eq("u8\"\"", Some("u8\"\""));

        // prefix-looking lead-ins that are NOT literals
        assert_eq("u", None);
        assert_eq("u8", None);
        assert_eq("L", None);
        assert_eq("Lx", None); // identifier, prefix not followed by quote
        assert_eq("u8x", None);
        assert_eq("l\"a\"", None); // lowercase l is not a valid prefix
        assert_eq("x\"a\"", None);

        // --- simple-escape-sequence ---
        assert_eq("\"\\n\"", Some("\"\\n\""));
        assert_eq("\"\\t\"", Some("\"\\t\""));
        assert_eq("\"\\r\"", Some("\"\\r\""));
        assert_eq("\"\\a\"", Some("\"\\a\""));
        assert_eq("\"\\b\"", Some("\"\\b\""));
        assert_eq("\"\\f\"", Some("\"\\f\""));
        assert_eq("\"\\v\"", Some("\"\\v\""));
        assert_eq("\"\\?\"", Some("\"\\?\""));
        assert_eq("\"\\\\\"", Some("\"\\\\\"")); // escaped backslash
        assert_eq("\"\\'\"", Some("\"\\'\"")); // escaped single-quote (valid in strings too)
        assert_eq("\"\\\"\"", Some("\"\\\"\"")); // escaped double-quote, then close (len 4)
        assert_eq("\"\\z\"", None); // \z is not a defined escape

        // --- octal-escape-sequence: \ then 1..3 octal digits ---
        assert_eq("\"\\0\"", Some("\"\\0\""));
        assert_eq("\"\\7\"", Some("\"\\7\""));
        assert_eq("\"\\12\"", Some("\"\\12\""));
        assert_eq("\"\\123\"", Some("\"\\123\""));
        assert_eq("\"\\1234\"", Some("\"\\1234\"")); // 4th octal digit is a plain s-char
        assert_eq("\"\\18\"", Some("\"\\18\"")); // 8 isn't octal -> ends escape, s-char

        // --- hexadecimal-escape-sequence: \x then 1+ hex digits ---
        assert_eq("\"\\x4\"", Some("\"\\x4\""));
        assert_eq("\"\\x41\"", Some("\"\\x41\""));
        assert_eq("\"\\xff\"", Some("\"\\xff\""));
        assert_eq("\"\\xABCD\"", Some("\"\\xABCD\"")); // hex is greedy, unbounded
        assert_eq("\"\\x\"", None); // \x requires >=1 hex digit
        assert_eq("\"\\xg\"", None);

        // --- universal-character-name: \u + 4 hex, \U + 8 hex ---
        assert_eq("\"\\u0041\"", Some("\"\\u0041\""));
        assert_eq("\"\\U00000041\"", Some("\"\\U00000041\""));
        assert_eq("\"\\u041\"", None); // only 3 hex
        assert_eq("\"\\u00G1\"", None); // non-hex mid-quad
        assert_eq("\"\\U0041\"", None); // \U needs 8
        assert_eq("\"\\U0000004\"", None); // 7 hex
        assert_eq("\"\\u00411\"", Some("\"\\u00411\"")); // extra hex past quad is an s-char

        // --- mixed body ---
        assert_eq("\"a\\n\\x41z\"", Some("\"a\\n\\x41z\""));

        // --- structural rejection ---
        assert_eq("\"", None); // lone opening quote
        assert_eq("\"a", None); // unterminated
        assert_eq("\"\\\"", None); // \" consumes the quote -> unterminated
        assert_eq("\"\\", None); // dangling backslash
        assert_eq("abc", None); // no opening quote
        assert_eq("123", None);
        assert_eq("", None); // empty input
        assert_eq(" \"a\"", None); // must match from char 0

        // --- newline / backslash-newline are not s-chars ---
        assert_eq("\"\n\"", None); // bare newline inside
        assert_eq("\"\\\n\"", None); // backslash-newline isn't an escape
    }

    #[test]
    fn identifier() {
        let assert_eq = assert_eq_wrapper(dfa_identifier);

        assert_eq("main() {", Some("main"));
        assert_eq("x + y", Some("x"));
        assert_eq("__internal2->next", Some("__internal2"));

        // keywords are a subset of identifiers
        assert_eq("int x;", Some("int"));
        assert_eq("return 0;", Some("return"));

        // digits are not in the start character set
        assert_eq("123abc", None);
        assert_eq("0x1F", None);

        assert_eq("", None);
        assert_eq(" main", None);
    }

    #[test]
    fn character_constant() {
        let assert_eq = assert_eq_wrapper(dfa_character_constant);

        // --- basic c-char ---
        assert_eq("'a'", Some("'a'"));
        assert_eq("'Z'", Some("'Z'"));
        assert_eq("'0'", Some("'0'"));
        assert_eq("' '", Some("' '")); // space is a valid c-char
        assert_eq("'\"'", Some("'\"'")); // a plain double-quote needs no escape

        // --- c-char-sequence is 1+ c-chars: multichar constants are legal ---
        assert_eq("'ab'", Some("'ab'"));
        assert_eq("'abcd'", Some("'abcd'"));

        // --- longest-match stops at the first closing quote ---
        assert_eq("'x' + 'y'", Some("'x'"));
        assert_eq("'a''b'", Some("'a'"));
        assert_eq("'a'b'", Some("'a'"));
        assert_eq("'a'; rest", Some("'a'"));

        // --- encoding-prefix: u8 | u | U | L ---
        assert_eq("u'a'", Some("u'a'"));
        assert_eq("U'a'", Some("U'a'"));
        assert_eq("L'a'", Some("L'a'"));
        assert_eq("u8'a'", Some("u8'a'"));

        // prefix-looking lead-ins that are NOT constants
        assert_eq("u", None);
        assert_eq("u8", None);
        assert_eq("L", None);
        assert_eq("Lx", None); // identifier, prefix not followed by quote
        assert_eq("u8x", None);
        assert_eq("l'a'", None); // lowercase l is not a valid prefix
        assert_eq("x'a'", None);

        // --- simple-escape-sequence ---
        assert_eq("'\\n'", Some("'\\n'"));
        assert_eq("'\\t'", Some("'\\t'"));
        assert_eq("'\\r'", Some("'\\r'"));
        assert_eq("'\\a'", Some("'\\a'"));
        assert_eq("'\\b'", Some("'\\b'"));
        assert_eq("'\\f'", Some("'\\f'"));
        assert_eq("'\\v'", Some("'\\v'"));
        assert_eq("'\\?'", Some("'\\?'"));
        assert_eq("'\\\\'", Some("'\\\\'")); // escaped backslash
        assert_eq("'\\''", Some("'\\''")); // escaped single-quote
        assert_eq("'\\\"'", Some("'\\\"'")); // escaped double-quote
        assert_eq("'\\z'", None); // \z is not a defined escape

        // --- octal-escape-sequence: \ then 1..3 octal digits ---
        assert_eq("'\\0'", Some("'\\0'"));
        assert_eq("'\\7'", Some("'\\7'"));
        assert_eq("'\\12'", Some("'\\12'"));
        assert_eq("'\\123'", Some("'\\123'"));
        assert_eq("'\\1234'", Some("'\\1234'")); // 4th octal digit is a plain c-char
        assert_eq("'\\18'", Some("'\\18'")); // 8 isn't octal -> ends escape, c-char

        // --- hexadecimal-escape-sequence: \x then 1+ hex digits ---
        assert_eq("'\\x4'", Some("'\\x4'"));
        assert_eq("'\\x41'", Some("'\\x41'"));
        assert_eq("'\\xff'", Some("'\\xff'"));
        assert_eq("'\\xABCD'", Some("'\\xABCD'")); // hex is greedy, unbounded
        assert_eq("'\\x'", None); // \x requires >=1 hex digit
        assert_eq("'\\xg'", None);

        // --- universal-character-name: \u + 4 hex, \U + 8 hex ---
        assert_eq("'\\u0041'", Some("'\\u0041'"));
        assert_eq("'\\U00000041'", Some("'\\U00000041'"));
        assert_eq("'\\u041'", None); // only 3 hex
        assert_eq("'\\u00G1'", None); // non-hex mid-quad
        assert_eq("'\\U0041'", None); // \U needs 8
        assert_eq("'\\U0000004'", None); // 7 hex
        assert_eq("'\\u00411'", Some("'\\u00411'")); // extra hex past quad is a c-char

        // --- structural rejection ---
        assert_eq("''", None); // empty: needs >=1 c-char
        assert_eq("'", None); // lone opening quote
        assert_eq("'a", None); // unterminated
        assert_eq("'\\'", None); // \' consumes the quote -> unterminated
        assert_eq("'\\", None); // dangling backslash
        assert_eq("abc", None); // no opening quote
        assert_eq("123", None);
        assert_eq("", None); // empty input
        assert_eq(" 'a'", None); // must match from char 0

        // --- newline / backslash-newline are not c-chars ---
        assert_eq("'\n'", None); // bare newline inside
        assert_eq("'\\\n'", None); // backslash-newline isn't an escape
    }

    #[test]
    fn punctuator() {
        let assert_eq = assert_eq_wrapper(dfa_punctuator);

        // --- single-char brackets / separators ---
        assert_eq("[", Some("["));
        assert_eq("]", Some("]"));
        assert_eq("(", Some("("));
        assert_eq(")", Some(")"));
        assert_eq("{", Some("{"));
        assert_eq("}", Some("}"));
        assert_eq(",", Some(","));
        assert_eq(";", Some(";"));
        assert_eq("~", Some("~"));
        assert_eq("?", Some("?"));

        // --- '.' vs '...' (note: '..' is NOT a punctuator) ---
        assert_eq(".", Some("."));
        assert_eq("...", Some("..."));
        assert_eq("..", Some(".")); // longest valid prefix is a single '.'
        assert_eq("....", Some("...")); // '...' then a separate '.'
        assert_eq(".x", Some("."));

        // --- '-' family: - -- -> -= ---
        assert_eq("-", Some("-"));
        assert_eq("--", Some("--"));
        assert_eq("->", Some("->"));
        assert_eq("-=", Some("-="));
        assert_eq("-x", Some("-"));

        // --- '+' family: + ++ += ---
        assert_eq("+", Some("+"));
        assert_eq("++", Some("++"));
        assert_eq("+=", Some("+="));
        assert_eq("+x", Some("+"));

        // --- '&' family: & && &= ---
        assert_eq("&", Some("&"));
        assert_eq("&&", Some("&&"));
        assert_eq("&=", Some("&="));

        // --- '|' family: | || |= ---
        assert_eq("|", Some("|"));
        assert_eq("||", Some("||"));
        assert_eq("|=", Some("|="));

        // --- single-op + '=' variants ---
        assert_eq("*", Some("*"));
        assert_eq("*=", Some("*="));
        assert_eq("/", Some("/"));
        assert_eq("/=", Some("/="));
        assert_eq("^", Some("^"));
        assert_eq("^=", Some("^="));
        assert_eq("!", Some("!"));
        assert_eq("!=", Some("!="));
        assert_eq("=", Some("="));
        assert_eq("==", Some("=="));

        // --- '<' family: < << <= <<= <: <% ---
        assert_eq("<", Some("<"));
        assert_eq("<<", Some("<<"));
        assert_eq("<=", Some("<="));
        assert_eq("<<=", Some("<<=")); // maximal munch: 3, not 2 or 1
        assert_eq("<<<", Some("<<")); // '<<' then a separate '<'
        assert_eq("<x", Some("<"));

        // --- '>' family: > >> >= >>= ---
        assert_eq(">", Some(">"));
        assert_eq(">>", Some(">>"));
        assert_eq(">=", Some(">="));
        assert_eq(">>=", Some(">>="));
        assert_eq(">>>", Some(">>"));

        // --- ':' family: : :: :> (C23 adds '::') ---
        assert_eq(":", Some(":"));
        assert_eq("::", Some("::"));
        assert_eq(":>", Some(":>")); // digraph for ']'
        assert_eq(":x", Some(":"));

        // --- '#' family: # ## ---
        assert_eq("#", Some("#"));
        assert_eq("##", Some("##"));
        assert_eq("###", Some("##")); // '##' then a separate '#'

        // --- '%' family incl. digraphs: % %= %> %: %:%: ---
        assert_eq("%", Some("%"));
        assert_eq("%=", Some("%="));
        assert_eq("%>", Some("%>")); // digraph for '}'
        assert_eq("%:", Some("%:")); // digraph for '#'
        assert_eq("%:%:", Some("%:%:")); // digraph for '##', length 4
        assert_eq("%:%", Some("%:")); // incomplete '%:%:' -> latch the '%:'
        assert_eq("%:%=", Some("%:")); // '%:' then a separate '%=' token

        // --- remaining digraphs ---
        assert_eq("<:", Some("<:")); // digraph for '['
        assert_eq("<%", Some("<%")); // digraph for '{'

        // --- rejections ---
        assert_eq("", None); // empty input
        assert_eq("a", None); // identifier-start, not a punctuator
        assert_eq("_x", None);
        assert_eq("0", None); // digit
        assert_eq("9", None);
        assert_eq(" +", None); // must match from char 0
        assert_eq("\t", None);
        assert_eq("\\", None); // backslash is not a punctuator
        assert_eq("@", None); // not in the source charset's punctuator set
        assert_eq("$", None);
        assert_eq("`", None);
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
            PPToken { text: &s[0..3], kind: PPKind::PPNumber },
            PPToken { text: &s[3..4], kind: PPKind::Punctuator },
            PPToken { text: &s[4..5], kind: PPKind::PPNumber },
            PPToken { text: &s[5..6], kind: PPKind::Punctuator },
            PPToken { text: &s[6..7], kind: PPKind::Identifier },
            PPToken { text: &s[7..8], kind: PPKind::Punctuator },
            PPToken { text: &s[8..9], kind: PPKind::Identifier },
            PPToken { text: &s[9..10], kind: PPKind::Punctuator },
            PPToken { text: &s[10..13], kind: PPKind::PPNumber },
            PPToken { text: &s[14..15], kind: PPKind::Punctuator },
            PPToken { text: &s[15..22], kind: PPKind::Identifier },
            PPToken { text: &s[23..30], kind: PPKind::HeaderName },
            PPToken { text: &s[31..32], kind: PPKind::Punctuator },
            PPToken { text: &s[32..38], kind: PPKind::Identifier },
            PPToken { text: &s[39..44], kind: PPKind::Identifier },
            PPToken { text: &s[44..45], kind: PPKind::Punctuator },
            PPToken { text: &s[45..51], kind: PPKind::Identifier },
            PPToken { text: &s[51..52], kind: PPKind::Other },
            PPToken { text: &s[52..53], kind: PPKind::Other },
        ];

        assert_eq!(result, expected, "{result:?} != {expected:?}");
    }

    #[test]
    fn pp_pluses() {
        const S: &str = r#"x+++++y"#;

        let s = S.chars().collect::<Vec<char>>();
        let result = pp_tokenize(&s);

        let expected = vec![
            PPToken { text: &s[0..1], kind: PPKind::Identifier },
            PPToken { text: &s[1..3], kind: PPKind::Punctuator },
            PPToken { text: &s[3..5], kind: PPKind::Punctuator },
            PPToken { text: &s[5..6], kind: PPKind::Punctuator },
            PPToken { text: &s[6..7], kind: PPKind::Identifier },
        ];

        assert_eq!(result, expected, "{result:?} != {expected:?}");
    }
}
