use std::{
    fmt::{self, Display},
    iter::Peekable,
    path::{Path, PathBuf},
};

use annotate_snippets::{AnnotationKind, Group, Renderer, Snippet};

pub use annotate_snippets::Level;

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
        self.display(&err.span, Some(format!("{:?}", err.inner)), Level::ERROR);
    }

    pub fn display(&self, span: &Span, msg: Option<String>, level: Level) {
        let path = &self.paths_opened[span.src];
        let content = &self.char_contents[span.src];
        show(path, content, span, msg, level);
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

    /// The smallest span covering both `self` and `other`: min `start`, max
    /// `end`. Assumes a shared `src` (a preprocessing-stage invariant), so
    /// `self.src` is kept as-is.
    pub fn union(&self, other: &Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
            src: self.src,
        }
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

fn show(path: impl AsRef<Path>, slice: &[char], span: &Span, msg: Option<String>, level: Level) {
    // `annotate-snippets` renders from a `&str` + byte ranges, but our source is
    // `&[char]` and spans are char indices (with `end` inclusive). Rebuild the
    // string and translate the char span into a byte range. Multi-line spans and
    // the color highlighting of the covered tokens are handled by the renderer.
    let source: String = slice.iter().collect();
    let byte_start: usize = slice[..span.start].iter().map(|c| c.len_utf8()).sum();
    let byte_end: usize = byte_start
        + slice[span.start..=span.end]
            .iter()
            .map(|c| c.len_utf8())
            .sum::<usize>();

    let path = path.as_ref().to_string_lossy();

    // The whole file is the source, so line 1 is line 1; `fold` elides the lines
    // that fall outside the span, keeping the old "just the relevant line(s)" look.
    let snippet = Snippet::source(source.as_str())
        .line_start(1)
        .path(path.as_ref())
        .fold(true)
        .annotation(
            AnnotationKind::Primary
                .span(byte_start..byte_end)
                .highlight_source(true),
        );

    let group = match &msg {
        Some(msg) => level.primary_title(msg.as_str()).element(snippet),
        None => Group::with_level(level).element(snippet),
    };

    eprintln!("{}", Renderer::styled().render(&[group]));
}
