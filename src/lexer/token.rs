use anyhow::anyhow;
use std::{fmt, str::FromStr as _};
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

#[derive(Debug, Clone, PartialEq)]
pub enum IntSuf {
    Blank,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FloatSuf {}

#[derive(Debug, Clone, PartialEq)]
pub enum CharEnc {}

#[derive(Debug, Clone, PartialEq)]
pub enum StrEnc {}

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
    Ident { value: String },
    IntConst { value: u64, suf: IntSuf },
    FloatConst { value: f64, suf: FloatSuf },
    CharConst { value: u32, enc: CharEnc },
    StrLit { value: String, enc: StrEnc },
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

/// `PartialEq` impl *ignores* self.span- for testing
#[derive(Clone)]
pub struct PPToken {
    pub text: String,
    pub kind: PPKind,
    pub nl: bool,
    pub ws: bool,
    pub span: Span,
}

impl fmt::Debug for PPToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

impl PPToken {
    pub fn promote(&self) -> Result<Token, Error> {
        let kind = match self.kind {
            PPKind::Header => unreachable!(),
            PPKind::Ident => match Kw::from_str(&self.text) {
                Ok(kw) => TokenKind::Kw(kw),
                Err(_) => TokenKind::Ident { value: self.text.clone() },
            },
            PPKind::PPNumber if let Ok(value) = u64::from_str(&self.text) => {
                TokenKind::IntConst { value, suf: IntSuf::Blank }
            }
            // PPKind::CharConst => {}
            // PPKind::StrLit => {}
            PPKind::Punct => {
                TokenKind::Punct(Punct::from_str(&self.text).expect("PPToken invariant bug'"))
            }
            // PPKind::UCN => {}
            // PPKind::Other => {}
            _ => {
                return Err(self
                    .span
                    .clone()
                    .into_error(anyhow!("unhandled preprocessing token")));
            }
        };

        Ok(Token { kind, span: self.span.clone() })
    }
}
