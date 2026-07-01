use std::borrow::Borrow;

use anyhow::Result;

use super::*;

pub fn dfa_header_name<T: Borrow<char>>(s: &[T]) -> Result<usize, String> {
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
        st = match (&st, c.borrow()) {
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

pub fn dfa_pp_number<T: Borrow<char>>(s: &[T]) -> Result<usize, String> {
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
        st = match (&st, c.borrow()) {
            (St::Ent, x) if DIGIT.contains(x) => St::Cont,
            (St::Ent, '.') => St::Period,
            (St::Period, x) if DIGIT.contains(x) => St::Cont,
            (St::Cont, x) if SIGN_LETTERS.contains(x) => St::SignLetter,
            (St::SignLetter, x) if SIGNS.contains(x) => St::Cont,
            (St::Cont | St::SignLetter, x) if IDENT_CONT.contains(x) => St::Cont,
            (St::Cont, '\'') => St::Apostrophe,
            (St::Apostrophe, x) if DIGIT_AND_NON_DIGIT.contains(x) => St::Cont,
            (St::Cont, '.') => St::Cont,
            _ => break,
        };

        if matches!(st, St::Cont | St::SignLetter) {
            result = Some(i)
        }

        last = Some(i);
    }

    result.map(|x| x + 1).ok_or(format!("{st:?}:{last:?}"))
}

pub fn dfa_character_constant<T: Borrow<char>>(s: &[T]) -> Result<usize, String> {
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
        st = match (&st, c.borrow()) {
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

pub fn dfa_ucn<T: Borrow<char>>(s: &[T]) -> Result<usize, String> {
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
        st = match (&st, c.borrow()) {
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

pub fn dfa_other<T: Borrow<char>>(s: &[T]) -> Result<usize, String> {
    match s.get(0).map(|x| x.borrow()) {
        // vertical-tab and form-feed
        Some(' ' | '\t' | '\n' | '\x0B' | '\x0C') => Err("Invalid".to_string()),
        Some(_) => Ok(1),
        None => Err("EOF".to_string()),
    }
}

// tokens: 7..=8

pub fn dfa_identifier<T: Borrow<char>>(s: &[T]) -> Result<usize, String> {
    #[derive(Debug)]
    enum St {
        Ent,
        Cont,
    }
    let mut st = St::Ent;
    let mut result = None;
    let mut last = None;

    for (i, c) in s.iter().enumerate() {
        st = match (&st, c.borrow()) {
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

pub fn dfa_string_literal<T: Borrow<char>>(s: &[T]) -> Result<usize, String> {
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
        st = match (&st, c.borrow()) {
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

pub fn dfa_punctuator<T: Borrow<char>>(s: &[T]) -> Result<usize, String> {
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
        st = match (&st, c.borrow()) {
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

pub fn dfa_comment<T: Borrow<char>>(s: &[T]) -> Result<usize, String> {
    #[derive(Debug)]
    enum St {
        Ent,
        Slash,
        LC,
        MLC,
        EMLC,
        Term,
    }
    let mut st = St::Ent;
    let mut result = None;
    let mut last = None;

    for (i, c) in s.iter().enumerate() {
        st = match (&st, c.borrow()) {
            (St::Ent, '/') => St::Slash,
            (St::Slash, '/') => St::LC,
            (St::LC, '\n') => break,
            (St::LC, _) => St::LC,
            (St::Slash, '*') => St::MLC,
            (St::MLC, '*') => St::EMLC,
            (St::MLC, _) => St::MLC,
            (St::EMLC, '/') => St::Term,
            (St::EMLC, _) => St::MLC,
            _ => break,
        };

        if matches!(st, St::LC | St::Term) {
            result = Some(i)
        }

        last = Some(i);
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
                "f({input:?}) -> {result:?} != {expected:?}",
            )
        }
    }

    #[test]
    fn comment() {
        let assert_eq = assert_eq_wrapper(dfa_comment);

        assert_eq("//  \n", Some("//  "));
        assert_eq("/**/  ", Some("/**/"));
        assert_eq(" //  \n", None);
        assert_eq(" /**/  ", None);
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
}
