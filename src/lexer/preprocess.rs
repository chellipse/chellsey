//! Translation phase 4: the preprocessing driver. Owns the macro table and
//! runs directives + macro expansion over the pp-token stream. The phase 1–3
//! text handling and the tokenizer itself live in `lexer.rs`; the macro
//! machinery (definitions, hide sets, expansion) in `directive.rs`.

use std::{fs::File, io::Read, path::Path};

use anyhow::Result;

use super::{PPKind, PPToken, directive, filter_esc_nl_and_rep_comments};
use crate::diagnostic::SourceManager;

pub struct Preprocessor<'a> {
    sm: &'a SourceManager,
    /// Living here (not threaded per call) is what lets an included file's
    /// definitions reach its includer across the recursive `run` calls.
    macros: directive::Macros,
}

impl<'a> Preprocessor<'a> {
    pub fn new(sm: &'a SourceManager) -> Self {
        Self { sm, macros: directive::Macros::default() }
    }

    /// Preprocess `path` and everything it includes into one pp-token stream.
    pub fn process(mut self, path: impl AsRef<Path>) -> Result<Vec<PPToken>> {
        // can't be None cause that's just if we've opened it before...
        let content = self.open(path)?.unwrap();
        self.run(&content)
    }

    /// Translation phases 1–3 for one file: read it, register it with the
    /// source manager, splice escaped newlines, replace comments with spaces.
    /// `None` means the file was already opened before.
    ///
    /// The chars borrow from the `SourceManager` (`'a`), not from `self` —
    /// which is what lets `run` recurse mutably while a file's content is
    /// still alive.
    fn open(&self, path: impl AsRef<Path>) -> Result<Option<Vec<&'a char>>> {
        let path = path.as_ref().to_path_buf();
        if !self.sm.is_opened(&path) {
            let mut f = File::open(&path)?;
            let mut buf = String::new();
            f.read_to_string(&mut buf)?;

            // phase 1
            let iter = self
                .sm
                .add_source(buf.chars().collect::<Vec<_>>().into_boxed_slice(), path);

            // phase 2 and partial phase 3, replacing comments with spaces
            let vec = filter_esc_nl_and_rep_comments(iter);

            return Ok(Some(vec));
        }

        Ok(None)
    }

    /// Translation phase 4: execute directives and expand macros, in one
    /// left-to-right scan (so definitions apply only to uses after them, and
    /// `#undef` takes effect mid-file).
    fn run(&mut self, content: &[&'a char]) -> Result<Vec<PPToken>> {
        let mut tokens = self.pp_tokenize(content);

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
                            let more_tokens = self.run(&content)?;
                            tokens.splice(i..i + 3, more_tokens.into_iter());
                        } else {
                            // already opened once: splice in nothing (a crude
                            // implicit `#pragma once`), but still drop the
                            // directive so the scan makes progress
                            tokens.drain(i..i + 3);
                        }
                    }
                    // TODO: make define w/o identifier error
                    [
                        PPToken { text, kind: PPKind::Ident, .. },
                        PPToken { text: key, kind: PPKind::Ident, nl: false, .. },
                        ..,
                    ] if text.as_str() == "define" => {
                        // the body is everything after the name to end of line
                        let body_at = i + 3;
                        let end = tokens[body_at..]
                            .iter()
                            .position(|x| x.nl)
                            .map_or(tokens.len(), |eol| body_at + eol);
                        let def = directive::DefineFn::new(&tokens[body_at..end])?;
                        // TODO: 6.10.5p2 — a redefinition must be identical;
                        // diagnose instead of overwriting
                        self.macros.insert(key.clone(), def);
                        tokens.drain(i..end);
                    }
                    [
                        PPToken { text, kind: PPKind::Ident, .. },
                        PPToken { text: key, kind: PPKind::Ident, nl: false, .. },
                        ..,
                    ] if text.as_str() == "undef" => {
                        self.macros.remove(key);
                        tokens.drain(i..i + 3);
                    }
                    x => {
                        todo!("TODO: {:?}", x);
                        // i += 1;
                    }
                };
            } else if !directive::try_expand_at(&mut tokens, i, &self.macros)? {
                i += 1;
            }
            // when try_expand_at expanded, stay at `i` and rescan the splice
        }

        Ok(tokens)
    }

    fn pp_tokenize(&self, s: &[&char]) -> Vec<PPToken> {
        let (src, content) = self.sm.latest();
        super::pp_tokenize(s, content, src)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run the full phase-1..4 pipeline (splice, comments, tokenize,
    /// directives + macro expansion) over a source string.
    fn preprocess_str(s: &str) -> Vec<PPToken> {
        let sm = SourceManager::new();
        let iter = sm.add_source(
            s.chars().collect::<Vec<_>>().into_boxed_slice(),
            std::path::PathBuf::from("test.c"),
        );
        let content = filter_esc_nl_and_rep_comments(iter);
        Preprocessor::new(&sm).run(&content).unwrap()
    }

    fn texts(tokens: &[PPToken]) -> Vec<&str> {
        tokens.iter().map(|t| t.text.as_str()).collect()
    }

    #[test]
    fn define_and_expand() {
        let result = preprocess_str(
            "#define TIMES(a, b) ((a) * (b))\n#define SIX TIMES(2, 3)\nint x = SIX;\n",
        );

        #[rustfmt::skip]
        assert_eq!(
            texts(&result),
            ["int", "x", "=", "(", "(", "2", ")", "*", "(", "3", ")", ")", ";"]
        );
    }

    #[test]
    fn define_undef() {
        let result = preprocess_str("#define A 1\nA\n#undef A\nA\n");
        assert_eq!(texts(&result), ["1", "A"]);
    }

    #[test]
    fn define_terminates() {
        // hide sets, end to end: the painted survivor stays put
        let result = preprocess_str("#define x x\nx\n");
        assert_eq!(texts(&result), ["x"]);
        assert!(result[0].hs.hides("x"));
    }

    /// C23 6.10.5.5 EXAMPLE 3 (the parts without `#`/`##`) — the canonical
    /// hide-set torture test: self-reference (`f`, `z`), reference through
    /// other macros (`g` → `f`, `t`, `m(m)`), a body with an unbalanced paren
    /// (`h`), commas produced by expansion *not* splitting arguments (`w`),
    /// invocations formed across splice boundaries (`h 5)`, `t(...)(1)`), and
    /// lazy bodies observing `#undef`/redefinition (`x`).
    ///
    /// The standard prints the expected expansion:
    /// ```c
    /// f(2 * (y+1)) + f(2 * (f(2 * (z[0])))) % f(2 * (0)) + t(1);
    /// f(2 * (2+(3,4)-0,1)) | f(2 * (~ 5)) & f(2 * (0,1))^m(0,1);
    /// ```
    ///
    /// The snapshot holds the rendered token stream (nl/ws-flag spacing, so
    /// whitespace is approximate) followed by every token with its hide set.
    #[test]
    fn expansion_torture() {
        let result = preprocess_str(
            "#define x 3\n\
             #define f(a) f(x * (a))\n\
             #undef x\n\
             #define x 2\n\
             #define g f\n\
             #define z z[0]\n\
             #define h g(~\n\
             #define m(a) a(w)\n\
             #define w 0,1\n\
             #define t(a) a\n\
             f(y+1) + f(f(z)) % t(t(g)(0) + t)(1);\n\
             g(x+(3,4)-w) | h 5) & m(f)^m(m);\n",
        );

        let mut rendered = String::new();
        for t in &result {
            if t.nl {
                rendered.push('\n');
            } else if t.ws {
                rendered.push(' ');
            }
            rendered.push_str(&t.text);
        }

        let listing: Vec<String> = result.iter().map(|t| format!("{t:?}")).collect();
        insta::assert_snapshot!(format!(
            "{}\n\n{}",
            rendered.trim_start(),
            listing.join("\n")
        ));
    }
}
