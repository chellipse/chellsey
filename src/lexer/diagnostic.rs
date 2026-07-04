use std::{
    collections::VecDeque,
    fmt::{Debug, Display, Write as _},
    path::Path,
};

#[derive(Debug, Clone)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub src: usize,
}

impl Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub fn show(path: impl AsRef<Path>, slice: &[char], span: &Span, msg: Option<String>) {
    let mut line_chars = VecDeque::with_capacity(2usize.pow(9));
    slice[span.start..=span.end]
        .iter()
        .for_each(|c| line_chars.push_back(c));

    let tlen = line_chars.len();

    for c in slice[..span.start].iter().rev() {
        match *c {
            '\n' => break,
            _ => line_chars.push_front(c),
        }
    }

    let col_no = line_chars.len() - tlen;

    if span.end < slice.len() {
        for c in slice[span.end + 1..].iter() {
            match *c {
                '\n' => break,
                _ => line_chars.push_back(c),
            }
        }
    }

    let line_no: u64 = slice[..span.start]
        .iter()
        .filter(|c| **c == '\n')
        .map(|_| 1)
        .sum();

    let line_no_str = format!("{line_no}");
    let lpad = line_no_str.len();

    let line_str = line_chars.into_iter().collect::<String>();

    let mut hl_str = String::new();
    hl_str.extend(std::iter::repeat(' ').take(col_no));
    hl_str.extend(std::iter::repeat('^').take(tlen));

    let mut buf = String::new();

    if let Some(msg) = msg {
        writeln!(buf, "{}", msg).unwrap();
    }
    writeln!(
        buf,
        "{:lpad$}--> {:?}:{line_no}:{col_no}",
        "",
        path.as_ref()
    )
    .unwrap();
    writeln!(buf, "{line_no_str} | {line_str}",).unwrap();
    writeln!(buf, "{:lpad$} | {hl_str}", "").unwrap();

    eprintln!("{buf}");
}
