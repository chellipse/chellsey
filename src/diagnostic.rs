use std::{
    collections::VecDeque,
    fmt::{self, Display, Write as _},
    iter::Peekable,
    path::{Path, PathBuf},
};

#[derive(Default)]
pub struct SourceManager {
    char_contents: elsa::FrozenVec<Box<[char]>>,
    paths_opened: elsa::index_set::FrozenIndexSet<PathBuf>,
}

impl SourceManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_source(
        &self,
        content: Box<[char]>,
        path: PathBuf,
    ) -> Peekable<impl Iterator<Item = &char>> {
        self.char_contents.push(content);
        self.paths_opened.insert(path);
        self.char_contents.last().unwrap().iter().peekable()
    }

    pub fn is_opened(&self, path: &PathBuf) -> bool {
        self.paths_opened.get(path).is_some()
    }

    pub fn latest(&self) -> (usize, &[char]) {
        let src = self.char_contents.len() - 1;
        (src, &self.char_contents[src])
    }

    pub fn report(&self, err: &Error) {
        let span = &err.span;
        let path = &self.paths_opened[span.src];
        let content = &self.char_contents[span.src];
        show(path, content, span, Some(format!("{:?}", err.inner)));
    }
}

#[derive(Debug, Clone)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub src: usize,
}

impl Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Span {
    pub fn into_error(self, e: impl Into<anyhow::Error>) -> Error {
        Error { span: self, inner: e.into() }
    }
}

#[derive(Debug)]
pub struct Error {
    pub span: Span,
    pub inner: anyhow::Error,
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl std::error::Error for Error {}

fn show(path: impl AsRef<Path>, slice: &[char], span: &Span, msg: Option<String>) {
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
