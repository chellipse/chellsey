use std::collections::HashMap;

use anyhow::{Result, anyhow};

use super::*;

/// The macro table, keyed by macro name. Threaded through preprocessing so
/// definitions from an included file apply to its includer.
pub type Macros = HashMap<String, DefineFn>;

#[derive(Debug)]
enum TokenOp {
    Transpose {
        arg_no: usize,
        offset: usize,
    },
    VaOpt {
        // represents the minimum number of args for this to be applied
        arg_no: usize,
        tokens: Vec<PPToken>,
        offset: usize,
    },
    VaArgs {
        // represented the start of a range
        arg_no: usize,
        offset: usize,
    },
}

impl TokenOp {
    /// `&vec[offset..]` should *include* the identifier token (ie define macro key)
    fn apply(&self, args: &[Vec<PPToken>], expr: &mut Vec<PPToken>) -> Result<()> {
        match self {
            Self::Transpose { arg_no, offset } => {
                expr.splice(
                    offset..offset,
                    args.get(*arg_no).ok_or(anyhow!("None"))?.iter().cloned(),
                );
            }
            Self::VaOpt { arg_no, tokens, offset } => {
                if args.len() > *arg_no {
                    expr.splice(offset..offset, tokens.iter().cloned());
                }
            }
            Self::VaArgs { arg_no, offset } => {
                // 6.10.5.1: __VA_ARGS__ is the trailing arguments merged into
                // one, *including* the separating commas — re-synthesized here
                // since `extract_args` splits them away
                if args.len() > *arg_no {
                    let mut joined: Vec<PPToken> = Vec::new();
                    for (n, arg) in args
                        .get(*arg_no..)
                        .ok_or(anyhow!("None"))?
                        .iter()
                        .enumerate()
                    {
                        // the separator's span is borrowed from a neighbor; it
                        // is never itself the subject of a diagnostic
                        if n > 0
                            && let Some(span) =
                                arg.first().or(joined.last()).map(|t| t.span.clone())
                        {
                            joined.push(PPToken {
                                text: COMMA.to_string(),
                                kind: PPKind::Punct,
                                nl: false,
                                ws: false,
                                span,
                                hs: HideSet::default(),
                            });
                        }
                        joined.extend(arg.iter().cloned());
                    }
                    expr.splice(offset..offset, joined);
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct DefineFn {
    /// distinguishes `#define F(..)` from `#define F ..` — a zero-parameter
    /// function-like macro must still consume its `()` at the call site, so
    /// this cannot be inferred from `procedures` being non-empty
    function_like: bool,
    procedures: Vec<TokenOp>,
    expr: Vec<PPToken>,
}

impl DefineFn {
    /// accepts the wildcard tokens as shown here, *not* the proceeding 3 constant tokens:
    /// `{#}{define}{IDENT}{?}*`
    ///
    /// `DefineFn` has zero knowledge of which identifier it belongs to, this is
    /// assumed to be handled by some higher level data structure
    pub fn new(line: &[PPToken]) -> Result<DefineFn> {
        if line.len() == 0 {
            return Ok(Self { function_like: false, procedures: Vec::new(), expr: Vec::new() });
        }

        if let PPToken { text, kind: PPKind::Punct, ws: false, .. } = &line[0]
            && text.as_str() == "("
        {
            let (used, params) = extract_args(line)?;
            // `()` extracts as a single empty parameter: a zero-parameter macro
            let params: Vec<&str> = if params.len() == 1 && params[0].is_empty() {
                Vec::new()
            } else if let Some(bad) = params.iter().find(|x| x.len() != 1) {
                // NOTE: identifier-list can only have identifiers separated by ',' -- 6.10.1
                return Err(anyhow!("Something wrong with these args: {:?}", bad));
            } else {
                params.iter().map(|x| x[0].text.as_str()).collect()
            };
            // the index of the first variadic argument, when the list ends `...`
            let arg_no = params.len().saturating_sub(1);
            let args = params;

            let mut expr = line[used + 1..].to_vec();

            let mut procedures = Vec::new();

            // scan *forward* so we don't have to do offset adjustment
            let mut offset = 0;
            loop {
                if let Some(token) = expr.get(offset) {
                    // TODO: this needs whitespace updating on sliced tokens
                    match token {
                        PPToken { text, kind: PPKind::Ident, .. } if text.as_str() == VA_OPT => {
                            if let Some(end) = match_paren(&expr[offset + 1..]) {
                                // should be only tokens between the parens
                                let tokens = expr[offset + 2..offset + end + 1].to_vec();
                                // println!("VA_OPT tokens: {tokens:?} {offset} {end} {expr:?}");
                                if let Some(split_pos) =
                                    tokens.iter().position(|x| x.text.as_str() == VA_ARGS)
                                {
                                    if let Some(slice) = tokens.get(..split_pos) {
                                        procedures.push(TokenOp::VaOpt {
                                            arg_no,
                                            tokens: slice.to_vec(),
                                            offset,
                                        });
                                    }
                                    procedures.push(TokenOp::VaArgs { arg_no, offset });
                                    if let Some(slice) = tokens.get(split_pos + 1..) {
                                        procedures.push(TokenOp::VaOpt {
                                            arg_no,
                                            tokens: slice.to_vec(),
                                            offset,
                                        });
                                    }
                                } else {
                                    procedures.push(TokenOp::VaOpt { arg_no, tokens, offset });
                                };
                                // should include VA_OPT and parens
                                expr.drain(offset..offset + end + 2);
                            };
                        }
                        PPToken { text, kind: PPKind::Ident, .. } if text.as_str() == VA_ARGS => {
                            procedures.push(TokenOp::VaArgs { arg_no, offset });
                            expr.remove(offset);
                        }
                        PPToken { text, kind: PPKind::Ident, .. }
                            if let Some(arg_no) = args.iter().position(|x| x == text) =>
                        {
                            procedures.push(TokenOp::Transpose { arg_no, offset });
                            expr.remove(offset);
                        }
                        _ => offset += 1,
                    }
                } else {
                    break;
                }
            }

            procedures.reverse();

            Ok(Self { function_like: true, procedures, expr })
        } else {
            Ok(Self {
                function_like: false,
                procedures: Vec::new(),
                expr: line.to_vec(),
            })
        }
    }

    pub fn is_function_like(&self) -> bool {
        self.function_like
    }

    /// accepts tokens starting with {IDENT} (assumed to be associted with this
    /// macro definition, unchecked), because then `Self::apply` can handle all
    /// token replacements
    ///
    /// `hs` is the hide set the expansion is painted with, computed by the
    /// caller (which knows the macro's name): `HS(name) ∪ {name}` for
    /// object-like, `(HS(name) ∩ HS(rparen)) ∪ {name}` for function-like.
    pub fn apply(
        &self,
        vec: &mut Vec<PPToken>,
        offset: usize,
        hs: &HideSet,
        macros: &Macros,
    ) -> Result<()> {
        let mut expr = self.expr.clone();

        let consumed = if self.function_like {
            let slice = vec.get(offset + 1..).ok_or(anyhow!("None"))?;
            let (used, mut args) = extract_args(slice)?;

            // argument prescan (6.10.5.1p11): each argument is fully
            // macro-expanded *before* substitution; painting then happens on
            // the substituted result. This ordering is what lets `f(f(1))`
            // expand fully while `f(f)(1)` correctly leaves the inner `f`
            // painted. (`#`/`##` will additionally need the unexpanded
            // argument copies — TBD.)
            for arg in &mut args {
                expand(arg, macros)?;
            }

            for procedure in &self.procedures {
                procedure.apply(&args, &mut expr)?;
            }

            used + 2 // the name, the arguments, and both parens
        } else {
            1 // just the name
        };

        // hsadd: paint every produced token so a rescan cannot re-enter this
        // expansion
        for token in &mut expr {
            token.hs = token.hs.union(hs);
        }

        vec.splice(offset..offset + consumed, expr);
        Ok(())
    }
}

/// Rescan `tokens` until no expandable macro invocation remains. Terminates
/// because every expansion paints its output: hide sets only grow.
pub fn expand(tokens: &mut Vec<PPToken>, macros: &Macros) -> Result<()> {
    let mut i = 0;
    while i < tokens.len() {
        if !try_expand_at(tokens, i, macros)? {
            i += 1;
        }
    }
    Ok(())
}

/// Expand the macro invocation starting at `tokens[i]`, if there is one.
/// Returns whether an expansion happened — the caller should rescan from `i`
/// (not advance) when it did.
pub fn try_expand_at(tokens: &mut Vec<PPToken>, i: usize, macros: &Macros) -> Result<bool> {
    if tokens[i].kind != PPKind::Ident || tokens[i].hs.hides(&tokens[i].text) {
        return Ok(false);
    }
    let Some(def) = macros.get(&tokens[i].text) else {
        return Ok(false);
    };

    let hs = if def.is_function_like() {
        // a function-like name is only an invocation when followed by `(`
        // (else it stays a plain identifier), and only when the call is
        // closed within the stream
        match tokens.get(i + 1) {
            Some(PPToken { text, kind: PPKind::Punct, .. }) if text == LPAREN => {}
            _ => return Ok(false),
        }
        let Some(rparen) = match_paren(&tokens[i + 1..]) else {
            return Ok(false);
        };
        tokens[i]
            .hs
            .intersect(&tokens[i + 1 + rparen].hs)
            .with(&tokens[i].text)
    } else {
        tokens[i].hs.with(&tokens[i].text)
    };

    def.apply(tokens, i, &hs, macros)?;
    Ok(true)
}

/// first token in input should be left paren
fn match_paren<'a>(tokens: &[PPToken]) -> Option<usize> {
    let mut depth = 0;
    let mut result = None;

    for (i, token) in tokens.iter().enumerate() {
        match token {
            PPToken { text, kind: PPKind::Punct, .. } if text == LPAREN => {
                depth += 1;
            }
            PPToken { text, kind: PPKind::Punct, .. } if text == RPAREN => {
                depth -= 1;
            }
            _ => {}
        }
        if depth == 0 {
            result = Some(i);
            break;
        }
    }

    result
}

fn extract_args<'a>(slice: &[PPToken]) -> Result<(usize, Vec<Vec<PPToken>>)> {
    let mut depth = 0;
    let mut args = Vec::new();
    let mut start = 1;
    let mut end = 0;
    let mut total = 0;

    for (i, token) in slice.iter().enumerate() {
        total = i;
        match token {
            PPToken { text, kind: PPKind::Punct, .. } if text == LPAREN => {
                depth += 1;
            }
            PPToken { text, kind: PPKind::Punct, .. } if text == RPAREN => {
                depth -= 1;
                if depth > 0 {
                    end = i;
                }
            }
            PPToken { text, kind: PPKind::Punct, .. } if text.as_str() == COMMA && depth == 1 => {
                args.push((start, end));
                start = i + 1;
                end = start;
            }
            _ => {
                end = i;
            }
        }

        if depth < 1 {
            break;
        }
    }

    args.push((start, end));

    let args = args
        .iter()
        .map(|(a, b)| slice[*a..=*b].to_vec())
        .collect::<Vec<_>>();

    Ok((total, args))
}

#[cfg(test)]
mod tests {
    use super::{super::tests::*, *};

    fn def_test_pair(d: &str, x: &str) -> Vec<PPToken> {
        let mut line = tokenize(d);
        line[0].ws = false;
        line[0].nl = false;
        let def = DefineFn::new(&line).unwrap();

        let mut vec = tokenize(x);
        def.apply(
            &mut vec,
            0,
            &HideSet::default().with("X"),
            &Macros::default(),
        )
        .unwrap();

        vec
    }

    /// Build a macro table from `(name, body)` pairs. A body string starting
    /// with an unspaced `(` is function-like, mirroring the real directive
    /// line.
    fn macros_of(defs: &[(&str, &str)]) -> Macros {
        defs.iter()
            .map(|(name, body)| {
                let mut line = tokenize(body);
                if let Some(first) = line.first_mut() {
                    first.nl = false;
                    first.ws = false;
                }
                (name.to_string(), DefineFn::new(&line).unwrap())
            })
            .collect()
    }

    fn expand_str(src: &str, macros: &Macros) -> String {
        let mut tokens = tokenize(src);
        expand(&mut tokens, macros).unwrap();
        let texts: Vec<_> = tokens.iter().map(|t| t.text.as_str()).collect();
        texts.join(" ")
    }

    #[test]
    fn hide_set_self_recursion() {
        let m = macros_of(&[("x", "x")]);
        assert_eq!(expand_str("x", &m), "x");
    }

    #[test]
    fn hide_set_mutual_recursion() {
        // valid C — must terminate, and the survivor is the painted name
        let m = macros_of(&[("a", "b"), ("b", "a")]);
        assert_eq!(expand_str("a", &m), "a");
        assert_eq!(expand_str("b", &m), "b");
    }

    #[test]
    fn nested_self_call() {
        // prescan: the inner f expands *before* the outer paints its output
        let m = macros_of(&[("f", "(x) (x)")]);
        assert_eq!(expand_str("f(f(1))", &m), "( ( 1 ) )");
    }

    #[test]
    fn painted_argument() {
        // f(f)(1): the argument `f` (no parens) survives prescan, then hsadd
        // paints it {f} — the rescan must NOT expand `f (1)`. gcc agrees.
        let m = macros_of(&[("f", "(x) x")]);
        assert_eq!(expand_str("f(f)(1)", &m), "f ( 1 )");
    }

    #[test]
    fn zero_param_function_like() {
        // `F()` is an invocation and consumes its parens; bare `F` is not
        let m = macros_of(&[("F", "() 42")]);
        assert_eq!(expand_str("F() + F", &m), "42 + F");
    }

    #[test]
    fn va_args_order_and_commas() {
        let m = macros_of(&[("F", "(a, ...) f(a, __VA_ARGS__)")]);
        assert_eq!(expand_str("F(1, 2, 3)", &m), "f ( 1 , 2 , 3 )");
    }

    #[test]
    fn define() {
        // object-like: replace
        assert_eq!(
            def_test_pair("42", "A"),
            vec![tok("42", PPKind::PPNumber, false, false)]
        );
        assert_eq!(
            def_test_pair("2 + 2", "A"),
            vec![
                tok("2", PPKind::PPNumber, false, false),
                tok("+", PPKind::Punct, false, true),
                tok("2", PPKind::PPNumber, false, true),
            ]
        );

        // function-like: argument substitution
        assert_eq!(
            def_test_pair("(A, B) B / A", "X(1, 2)"),
            vec![
                tok("2", PPKind::PPNumber, false, true),
                tok("/", PPKind::Punct, false, true),
                tok("1", PPKind::PPNumber, false, false),
            ]
        );
        assert_eq!(
            def_test_pair("(A, B) B / A / B", "X(1, 2)"),
            vec![
                tok("2", PPKind::PPNumber, false, true),
                tok("/", PPKind::Punct, false, true),
                tok("1", PPKind::PPNumber, false, false),
                tok("/", PPKind::Punct, false, true),
                tok("2", PPKind::PPNumber, false, true),
            ]
        );
        assert_eq!(
            def_test_pair("(A, B) B / A", "X(1, 2 + 2)"),
            vec![
                tok("2", PPKind::PPNumber, false, true),
                tok("+", PPKind::Punct, false, true),
                tok("2", PPKind::PPNumber, false, true),
                tok("/", PPKind::Punct, false, true),
                tok("1", PPKind::PPNumber, false, false),
            ]
        );
        assert_eq!(
            def_test_pair("(A, B) B / A", "X(1 * 1, 2 + 2)"),
            vec![
                tok("2", PPKind::PPNumber, false, true),
                tok("+", PPKind::Punct, false, true),
                tok("2", PPKind::PPNumber, false, true),
                tok("/", PPKind::Punct, false, true),
                tok("1", PPKind::PPNumber, false, false),
                tok("*", PPKind::Punct, false, true),
                tok("1", PPKind::PPNumber, false, true),
            ]
        );

        // variadic: __VA_OPT__ / __VA_ARGS__
        assert_eq!(
            def_test_pair("(...) f(0 __VA_OPT__(,) __VA_ARGS__)", "X(1)"),
            vec![
                tok("f", PPKind::Ident, false, true),
                tok("(", PPKind::Punct, false, false),
                tok("0", PPKind::PPNumber, false, false),
                tok(",", PPKind::Punct, false, false),
                tok("1", PPKind::PPNumber, false, false),
                tok(")", PPKind::Punct, false, false),
            ]
        );
        assert_eq!(
            def_test_pair(
                "(sname, ...) S sname __VA_OPT__(= { __VA_ARGS__ })",
                "X(xxx, 123)"
            ),
            vec![
                tok("S", PPKind::Ident, false, true),
                tok("xxx", PPKind::Ident, false, false),
                tok("=", PPKind::Punct, false, false),
                tok("{", PPKind::Punct, false, true),
                tok("123", PPKind::PPNumber, false, true),
                tok("}", PPKind::Punct, false, true),
            ]
        );
    }

    #[test]
    fn arg_extraction() {
        const S: &str = r#"(a, 1 + 2, b, f(c), xXx)"#;
        let tokens = tokenize(S);
        let (_, args) = extract_args(&tokens).unwrap();

        assert_eq!(
            args,
            vec![
                vec![tok("a", PPKind::Ident, false, false)],
                vec![
                    tok("1", PPKind::PPNumber, false, true),
                    tok("+", PPKind::Punct, false, true),
                    tok("2", PPKind::PPNumber, false, true),
                ],
                vec![tok("b", PPKind::Ident, false, true)],
                vec![
                    tok("f", PPKind::Ident, false, true),
                    tok("(", PPKind::Punct, false, false),
                    tok("c", PPKind::Ident, false, false),
                    tok(")", PPKind::Punct, false, false),
                ],
                vec![tok("xXx", PPKind::Ident, false, true)],
            ]
        );
    }
}
