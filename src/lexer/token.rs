use anyhow::anyhow;
use std::{fmt, rc::Rc, str::FromStr as _};
use strum_macros::EnumString;

use crate::diagnostic::{Error, Span};

#[derive(Debug, Clone, PartialEq, EnumString)]
#[allow(non_camel_case_types)]
pub enum Kw {
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
    #[strum(serialize = "while")]
    _while,
    _Atomic,
    _BitInt,
    _Complex,
    _Decimal32,
    _Decimal64,
    _Decimal128,
    _Generic,
    _Imaginary,
    _Noreturn,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IntSuf {
    pub unsigned: bool,
    pub len: IntLen,
}

/// The length part of an integer-suffix (6.4.4.1) — the *minimum* type the
/// constant gets; typing may still escalate by magnitude. Ordered so rank
/// comparisons work. A `BitPrecise(u16)` variant slots in when `_BitInt`
/// becomes real.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IntLen {
    /// no length suffix
    Int,
    /// `l` / `L`
    Long,
    /// `ll` / `LL`
    LongLong,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FloatSuf {
    Double,
    Float,
    LongDouble,
}

/// `L`/`u`/`u8`-prefixed literals are TBD; the enum exists so adding them is
/// a variant, not a reshape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CharEnc {
    Plain,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StrEnc {
    Plain,
}

#[derive(Debug, Clone, PartialEq, EnumString)]
pub enum Punct {
    #[strum(serialize = "[", serialize = "<:")]
    LBracket,
    #[strum(serialize = "]", serialize = ":>")]
    RBracket,
    #[strum(serialize = "(")]
    LParen,
    #[strum(serialize = ")")]
    RParen,
    #[strum(serialize = "{", serialize = "<%")]
    LBrace,
    #[strum(serialize = "}", serialize = "%>")]
    RBrace,
    #[strum(serialize = ".")]
    Dot,
    #[strum(serialize = "->")]
    Arrow,
    #[strum(serialize = "++")]
    PlusPlus,
    #[strum(serialize = "--")]
    MinusMinus,
    #[strum(serialize = "&")]
    Amp,
    #[strum(serialize = "*")]
    Star,
    #[strum(serialize = "+")]
    Plus,
    #[strum(serialize = "-")]
    Minus,
    #[strum(serialize = "~")]
    Tilde,
    #[strum(serialize = "!")]
    Bang,
    #[strum(serialize = "/")]
    Slash,
    #[strum(serialize = "%")]
    Percent,
    #[strum(serialize = "<<")]
    LtLt,
    #[strum(serialize = ">>")]
    GtGt,
    #[strum(serialize = "<")]
    Lt,
    #[strum(serialize = ">")]
    Gt,
    #[strum(serialize = "<=")]
    LtEq,
    #[strum(serialize = ">=")]
    GtEq,
    #[strum(serialize = "==")]
    EqEq,
    #[strum(serialize = "!=")]
    BangEq,
    #[strum(serialize = "^")]
    Caret,
    #[strum(serialize = "|")]
    Pipe,
    #[strum(serialize = "&&")]
    AmpAmp,
    #[strum(serialize = "||")]
    PipePipe,
    #[strum(serialize = "?")]
    Question,
    #[strum(serialize = ":")]
    Colon,
    #[strum(serialize = "::")]
    ColonColon,
    #[strum(serialize = ";")]
    SemiColon,
    #[strum(serialize = "...")]
    Ellipsis,
    #[strum(serialize = "=")]
    Eq,
    #[strum(serialize = "*=")]
    StarEq,
    #[strum(serialize = "/=")]
    SlashEq,
    #[strum(serialize = "%=")]
    PercentEq,
    #[strum(serialize = "+=")]
    PlusEq,
    #[strum(serialize = "-=")]
    MinusEq,
    #[strum(serialize = "<<=")]
    LtLtEq,
    #[strum(serialize = ">>=")]
    GtGtEq,
    #[strum(serialize = "&=")]
    AmpEq,
    #[strum(serialize = "^=")]
    CaretEq,
    #[strum(serialize = "|=")]
    PipeEq,
    #[strum(serialize = ",")]
    Comma,
    #[strum(serialize = "#", serialize = "%:")]
    Hash,
    #[strum(serialize = "##", serialize = "%:%:")]
    HashHash,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Kw(Kw),
    Ident {
        value: String,
    },
    IntConst {
        value: u64,
        suf: IntSuf,
    },
    FloatConst {
        value: f64,
        suf: FloatSuf,
    },
    CharConst {
        value: u32,
        enc: CharEnc,
    },
    /// decoded raw bytes (escapes resolved, UCNs UTF-8-encoded), not text;
    /// NUL termination is added when the middle-end interns it
    StrLit {
        value: Vec<u8>,
        enc: StrEnc,
    },
    Punct(Punct),
}

#[derive(Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Token").field(&self.kind).finish()
    }
}

// preprocessing-tokens: 3..=6

/// The macro names a token may no longer be expanded by — 6.10.5.4p2's
/// non-replacement rule, tracked per token (Prosser's "hide set"). Expanding
/// macro `M` paints every produced token with `M`; a painted token never
/// re-expands, which is what makes `#define x x` — and mutual recursion
/// through any number of macros — terminate with no fuel counter.
///
/// Persistent cons list: `with` is O(1) and shares the tail, so painting the
/// N tokens of one expansion with the same set is N `Rc` bumps. Sets are tiny
/// (one name per level of active expansion), so the O(n) `hides` walk is fine.
#[derive(Clone, Default)]
pub struct HideSet(Option<Rc<HsNode>>);

struct HsNode {
    name: String,
    rest: Option<Rc<HsNode>>,
}

impl HideSet {
    pub fn hides(&self, name: &str) -> bool {
        let mut cur = &self.0;
        while let Some(node) = cur {
            if node.name == name {
                return true;
            }
            cur = &node.rest;
        }
        false
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_none()
    }

    pub fn with(&self, name: &str) -> HideSet {
        if self.hides(name) {
            return self.clone();
        }
        HideSet(Some(Rc::new(HsNode {
            name: name.to_string(),
            rest: self.0.clone(),
        })))
    }

    /// The names present in both — the function-like invocation rule is
    /// `(HS(name) ∩ HS(rparen)) ∪ {name}`.
    pub fn intersect(&self, other: &HideSet) -> HideSet {
        let mut out = HideSet::default();
        let mut cur = &self.0;
        while let Some(node) = cur {
            if other.hides(&node.name) {
                out = out.with(&node.name);
            }
            cur = &node.rest;
        }
        out
    }

    pub fn union(&self, other: &HideSet) -> HideSet {
        let mut out = self.clone();
        let mut cur = &other.0;
        while let Some(node) = cur {
            out = out.with(&node.name);
            cur = &node.rest;
        }
        out
    }
}

impl fmt::Debug for HideSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut set = f.debug_set();
        let mut cur = &self.0;
        while let Some(node) = cur {
            set.entry(&node.name);
            cur = &node.rest;
        }
        set.finish()
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum PPKind {
    Header,
    Ident,
    PPNumber,
    CharConst,
    StrLit,
    Punct,
    UCN,
    Other,
}

/// `PartialEq` impl *ignores* self.span and self.hs - for testing
#[derive(Clone)]
pub struct PPToken {
    pub text: String,
    pub kind: PPKind,
    pub nl: bool,
    pub ws: bool,
    pub span: Span,
    pub hs: HideSet,
}

impl fmt::Debug for PPToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.hs.is_empty() {
            write!(f, "{{{}:{:?}}}", self.text, self.kind)
        } else {
            write!(f, "{{{}:{:?}:{:?}}}", self.text, self.kind, self.hs)
        }
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

impl PPToken {
    pub fn promote(&self) -> Result<Token, Error> {
        let kind = match self.kind {
            PPKind::Header => unreachable!(),
            PPKind::Ident => match Kw::from_str(&self.text) {
                Ok(kw) => TokenKind::Kw(kw),
                Err(_) => TokenKind::Ident { value: self.text.clone() },
            },
            PPKind::PPNumber => self.convert_pp_number()?,
            PPKind::CharConst => self.convert_char_const()?,
            PPKind::StrLit => self.convert_str_lit()?,
            PPKind::Punct => {
                TokenKind::Punct(Punct::from_str(&self.text).expect("PPToken invariant bug'"))
            }
            PPKind::UCN | PPKind::Other => {
                return Err(self.err("unhandled preprocessing token"));
            }
        };

        Ok(Token { kind, span: self.span.clone() })
    }

    pub(crate) fn err(&self, msg: impl fmt::Display) -> Error {
        self.span.clone().into_error(anyhow!("{msg}"))
    }

    /// 6.4.4.1 / 6.4.4.2: dispatch a pp-number to integer vs floating
    /// conversion by shape. Digit separators (`'`) are stripped up front.
    /// `pub(crate)` for the `#if` evaluator's `atom()`.
    pub(crate) fn convert_pp_number(&self) -> Result<TokenKind, Error> {
        let text: String = self.text.chars().filter(|&c| c != '\'').collect();
        let t = text.as_str();

        if let Some(rest) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
            if rest.contains(['.', 'p', 'P']) {
                self.convert_float(t, true)
            } else {
                self.convert_int(rest, 16)
            }
        } else if t.contains(['.', 'e', 'E']) {
            // checked before the octal prefix: `0.5` / `0e1` are decimal floats
            self.convert_float(t, false)
        } else if let Some(rest) = t.strip_prefix("0b").or_else(|| t.strip_prefix("0B")) {
            self.convert_int(rest, 2)
        } else if t.len() > 1 && t.starts_with('0') {
            self.convert_int(&t[1..], 8)
        } else {
            self.convert_int(t, 10)
        }
    }

    /// `s` is digits-then-suffix, radix prefix already stripped.
    fn convert_int(&self, s: &str, radix: u32) -> Result<TokenKind, Error> {
        let mut value: u64 = 0;
        let mut end = 0;
        for (i, c) in s.char_indices() {
            let Some(d) = c.to_digit(radix) else { break };
            value = value
                .checked_mul(radix as u64)
                .and_then(|v| v.checked_add(d as u64))
                .ok_or_else(|| self.err("integer constant is too large"))?;
            end = i + c.len_utf8();
        }
        // an empty digit run is only valid for octal: `0` and e.g. `0U`
        // arrive here as "" after their leading `0` was taken as the prefix
        if end == 0 && radix != 8 {
            return Err(self.err("integer constant needs digits"));
        }
        let suf = self.parse_int_suf(&s[end..])?;
        Ok(TokenKind::IntConst { value, suf })
    }

    fn convert_float(&self, t: &str, hex: bool) -> Result<TokenKind, Error> {
        // suffix chars can't occur in a valid body: decimal bodies hold
        // digits/./e/±, and a hex mantissa `f` only appears in constants that
        // are missing their (required) `p` exponent and error anyway
        let (body, suf) = if t.ends_with(['f', 'F']) {
            (&t[..t.len() - 1], FloatSuf::Float)
        } else if t.ends_with(['l', 'L']) {
            (&t[..t.len() - 1], FloatSuf::LongDouble)
        } else {
            (t, FloatSuf::Double)
        };

        let value = if hex {
            self.parse_hex_float(body)?
        } else {
            body.parse::<f64>()
                .map_err(|_| self.err("invalid floating constant"))?
        };
        Ok(TokenKind::FloatConst { value, suf })
    }

    /// `0x h.h p±d` (6.4.4.2): mantissa hex digits accumulate into a u128
    /// (exact well past f64's 53 bits — overflow only drops sub-ulp digits),
    /// then one correctly-rounded `as f64` and an exact power-of-two scale.
    fn parse_hex_float(&self, s: &str) -> Result<f64, Error> {
        let s = &s[2..]; // the 0x prefix
        let (mant, exp) = s
            .split_once(['p', 'P'])
            .ok_or_else(|| self.err("hexadecimal floating constants require a `p` exponent"))?;
        let (int_part, frac_part) = mant.split_once('.').unwrap_or((mant, ""));
        if int_part.is_empty() && frac_part.is_empty() {
            return Err(self.err("hexadecimal floating constant needs mantissa digits"));
        }

        let mut m: u128 = 0;
        let mut scale: i32 = 0;
        for (c, frac) in int_part
            .chars()
            .zip(std::iter::repeat(false))
            .chain(frac_part.chars().zip(std::iter::repeat(true)))
        {
            let d = c
                .to_digit(16)
                .ok_or_else(|| self.err(format!("invalid hex digit `{c}`")))?
                as u128;
            if m > (u128::MAX - 15) / 16 {
                // saturated: dropped integer digits still scale the value;
                // dropped fraction digits are beyond representable precision
                if !frac {
                    scale += 4;
                }
            } else {
                m = m * 16 + d;
                if frac {
                    scale -= 4;
                }
            }
        }

        let (sign, digits) = match exp.as_bytes().first() {
            Some(b'+') => (1i32, &exp[1..]),
            Some(b'-') => (-1, &exp[1..]),
            _ => (1, exp),
        };
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err(self.err("invalid exponent in hexadecimal floating constant"));
        }
        let e: i32 = digits
            .parse()
            .map_err(|_| self.err("floating constant exponent out of range"))?;

        if m == 0 {
            return Ok(0.0);
        }
        // beyond ±5000 the result is inf/0 regardless (f64 spans ~2^±1074)
        let e = (sign * e).saturating_add(scale).clamp(-5000, 5000);
        Ok(m as f64 * 2f64.powi(e))
    }

    /// The contents between the quotes. Errors on encoding prefixes (TBD).
    fn literal_body(&self, quote: char) -> Result<&str, Error> {
        let s = self.text.as_str();
        if !s.starts_with(quote) {
            return Err(self.err("encoding-prefixed literals are TBD"));
        }
        Ok(&s[1..s.len() - 1])
    }

    /// 6.4.4.4 / 6.4.5 escape decoding, shared by char constants and string
    /// literals. Numeric escapes are raw bytes; source characters and UCNs
    /// are code points (UTF-8-encoded when they land in a string).
    fn decode_escapes(&self, body: &str) -> Result<Vec<CChar>, Error> {
        let mut out = Vec::new();
        let mut chars = body.chars().peekable();
        while let Some(c) = chars.next() {
            if c != '\\' {
                out.push(CChar::Char(c));
                continue;
            }
            let e = chars
                .next()
                .ok_or_else(|| self.err("lone `\\` in literal"))?;
            out.push(match e {
                '\'' => CChar::Byte(b'\''),
                '"' => CChar::Byte(b'"'),
                '?' => CChar::Byte(b'?'),
                '\\' => CChar::Byte(b'\\'),
                'a' => CChar::Byte(7),
                'b' => CChar::Byte(8),
                'f' => CChar::Byte(12),
                'n' => CChar::Byte(b'\n'),
                'r' => CChar::Byte(b'\r'),
                't' => CChar::Byte(b'\t'),
                'v' => CChar::Byte(11),
                'x' => {
                    let mut v: u32 = 0;
                    let mut any = false;
                    while let Some(d) = chars.peek().and_then(|c| c.to_digit(16)) {
                        chars.next();
                        any = true;
                        v = v
                            .checked_mul(16)
                            .and_then(|v| v.checked_add(d))
                            .filter(|&v| v <= 0xFF)
                            .ok_or_else(|| self.err("hex escape out of range"))?;
                    }
                    if !any {
                        return Err(self.err("`\\x` needs hex digits"));
                    }
                    CChar::Byte(v as u8)
                }
                'u' | 'U' => {
                    let mut v: u32 = 0;
                    for _ in 0..if e == 'u' { 4 } else { 8 } {
                        let d = chars
                            .next()
                            .and_then(|c| c.to_digit(16))
                            .ok_or_else(|| self.err("truncated universal character name"))?;
                        v = v.wrapping_mul(16).wrapping_add(d);
                    }
                    // 6.4.3: below A0 only $ @ ` are allowed; from_u32
                    // rejects surrogates and > 0x10FFFF
                    if v < 0xA0 && !matches!(v, 0x24 | 0x40 | 0x60) {
                        return Err(self.err("universal character name out of range"));
                    }
                    CChar::Char(
                        char::from_u32(v)
                            .ok_or_else(|| self.err("invalid universal character name"))?,
                    )
                }
                o if o.is_digit(8) => {
                    // at most 3 octal digits: `\0001` is `\000` then `1`
                    let mut v = o.to_digit(8).unwrap();
                    for _ in 0..2 {
                        let Some(d) = chars.peek().and_then(|c| c.to_digit(8)) else {
                            break;
                        };
                        chars.next();
                        v = v * 8 + d;
                    }
                    if v > 0xFF {
                        return Err(self.err("octal escape out of range"));
                    }
                    CChar::Byte(v as u8)
                }
                _ => return Err(self.err(format!("unknown escape `\\{e}`"))),
            });
        }
        Ok(out)
    }

    /// `pub(crate)` for the `#if` evaluator's `atom()`.
    pub(crate) fn convert_char_const(&self) -> Result<TokenKind, Error> {
        let body = self.literal_body('\'')?;
        let decoded = self.decode_escapes(body)?;
        let [c] = decoded.as_slice() else {
            return Err(self.err("character constant must hold exactly one character (TBD)"));
        };
        let value = match c {
            CChar::Byte(b) => *b as u32,
            CChar::Char(c) => *c as u32,
        };
        Ok(TokenKind::CharConst { value, enc: CharEnc::Plain })
    }

    fn convert_str_lit(&self) -> Result<TokenKind, Error> {
        let body = self.literal_body('"')?;
        let mut value = Vec::new();
        for c in self.decode_escapes(body)? {
            match c {
                CChar::Byte(b) => value.push(b),
                CChar::Char(c) => value.extend_from_slice(c.encode_utf8(&mut [0; 4]).as_bytes()),
            }
        }
        Ok(TokenKind::StrLit { value, enc: StrEnc::Plain })
    }

    /// Integer-suffix per 6.4.4.1: at most one `u`/`U` and one length
    /// suffix, in either order. The valid spellings are enumerated raw —
    /// the doubled spellings are same-case only (`ll`/`LL`, never `lL`),
    /// which bounds the table, and mixed case then falls out as invalid
    /// with no casing logic at all.
    #[rustfmt::skip]
    fn parse_int_suf(&self, s: &str) -> Result<IntSuf, Error> {
        let (unsigned, len) = match s {
            ""                                                        => (false, IntLen::Int),
            "u" | "U"                                                 => (true,  IntLen::Int),
            "l" | "L"                                                 => (false, IntLen::Long),
            "ul" | "uL" | "Ul" | "UL" | "lu" | "lU" | "Lu" | "LU"     => (true,  IntLen::Long),
            "ll" | "LL"                                               => (false, IntLen::LongLong),
            "ull" | "uLL" | "Ull" | "ULL" | "llu" | "llU" | "LLu" | "LLU" => (true, IntLen::LongLong),
            // valid C23, distinct from a junk suffix — but nothing downstream
            // models `_BitInt`, so a clear error beats a silently-wrong `int`
            "wb" | "WB" | "uwb" | "uWB" | "Uwb" | "UWB" | "wbu" | "wbU" | "WBu" | "WBU" => {
                return Err(self.err("`wb` (`_BitInt`) integer constants are TBD"));
            }
            _ => return Err(self.err(format!("invalid integer constant suffix `{s}`"))),
        };
        Ok(IntSuf { unsigned, len })
    }
}

/// One decoded element of a char constant or string literal.
enum CChar {
    /// a numeric escape — a raw byte, no encoding applied
    Byte(u8),
    /// a source character or UCN — a code point
    Char(char),
}
