use anyhow::{anyhow, Result};

use super::*;

#[derive(Debug)]
enum TokenOp<'a> {
    Transpose {
        arg_no: usize,
        offset: usize,
    },
    VaOpt {
        // represents the minimum number of args for this to be applied
        arg_no: usize,
        tokens: Vec<PPToken<'a>>,
        offset: usize,
    },
    VaArgs {
        // represented the start of a range
        arg_no: usize,
        offset: usize,
    },
}

impl<'a> TokenOp<'a> {
    /// `&vec[offset..]` should *include* the identifier token (ie define macro key)
    fn apply(&self, args: &[Vec<PPToken<'a>>], expr: &mut Vec<PPToken<'a>>) -> Result<()> {
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
                if args.len() > *arg_no {
                    for arg in args.get(*arg_no..).ok_or(anyhow!("None"))? {
                        expr.splice(offset..offset, arg.iter().cloned());
                    }
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct DefineFn<'a> {
    procedures: Option<Vec<TokenOp<'a>>>,
    expr: Vec<PPToken<'a>>,
}

impl<'a> DefineFn<'a> {
    /// accepts the wildcard tokens as shown here, *not* the proceeding 3 constant tokens:
    /// `{#}{define}{IDENT}{?}*`
    ///
    /// `DefineFn` has zero knowledge of which identifier it belongs to, this is
    /// assumed to be handled by some higher level data structure
    pub fn new(line: &[PPToken<'a>]) -> Result<DefineFn<'a>> {
        if line.len() == 0 {
            return Ok(Self { procedures: None, expr: Vec::new() });
        }

        if let PPToken { text: ['('], kind: PPKind::Punct, ws: false, .. } = line[0] {
            let (used, args) = extract_args(line)?;
            // NOTE: identifier-list can only have identifiers separated by ',' -- 6.10.1
            if args.iter().find(|x| x.len() != 1).is_some() {
                return Err(anyhow!("Something wrong with these args: {:?}", args));
            };
            let args: Vec<&[char]> = args.iter().map(|x| x[0].text).collect();
            let arg_no = args.len() - 1;

            let mut expr = line[used + 1..].to_vec();

            let mut procedures = Vec::new();

            // scan *forward* so we don't have to do offset adjustment
            let mut offset = 0;
            loop {
                if let Some(token) = expr.get(offset) {
                    // TODO: this needs whitespace updating on sliced tokens
                    match token {
                        PPToken { text: VA_OPT, kind: PPKind::Ident, .. } => {
                            if let Some(end) = match_paren(&expr[offset + 1..]) {
                                // should be only tokens between the parens
                                let tokens = expr[offset + 2..offset + end + 1].to_vec();
                                // println!("VA_OPT tokens: {tokens:?} {offset} {end} {expr:?}");
                                if let Some(split_pos) =
                                    tokens.iter().position(|x| x.text == VA_ARGS)
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
                        PPToken { text: VA_ARGS, kind: PPKind::Ident, .. } => {
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

            let procedures = Some(procedures).filter(|x| x.len() > 0);

            Ok(Self { procedures, expr })
        } else {
            Ok(Self { procedures: None, expr: line.to_vec() })
        }
    }

    /// accepts tokens starting with {IDENT} (assumed to be associted with this
    /// macro definition, unchecked), because then `Self::apply` can handle all
    /// token replacements
    pub fn apply(&self, vec: &mut Vec<PPToken<'a>>, offset: usize) -> Result<()> {
        if let Some(procedures) = &self.procedures {
            let slice = vec.get(offset + 1..).ok_or(anyhow!("None"))?;
            let (used, args) = extract_args(slice)?;

            let mut expr = self.expr.clone();

            for procedure in procedures {
                procedure.apply(&args, &mut expr)?;
            }

            vec.splice(offset..offset + used + 2, expr);
        } else {
            vec.splice(offset..offset + 1, self.expr.clone());
        }

        Ok(())
    }
}

/// first token in input should be left paren
fn match_paren<'a>(tokens: &[PPToken<'a>]) -> Option<usize> {
    let mut depth = 0;
    let mut result = None;

    for (i, token) in tokens.iter().enumerate() {
        match token {
            PPToken { text: ['('], kind: PPKind::Punct, .. } => {
                depth += 1;
            }
            PPToken { text: [')'], kind: PPKind::Punct, .. } => {
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

fn extract_args<'a>(slice: &[PPToken<'a>]) -> Result<(usize, Vec<Vec<PPToken<'a>>>)> {
    let mut depth = 0;
    let mut args = Vec::new();
    let mut start = 1;
    let mut end = 0;
    let mut total = 0;

    for (i, token) in slice.iter().enumerate() {
        total = i;
        match token {
            PPToken { text: ['('], kind: PPKind::Punct, .. } => {
                depth += 1;
            }
            PPToken { text: [')'], kind: PPKind::Punct, .. } => {
                depth -= 1;
                if depth > 0 {
                    end = i;
                }
            }
            PPToken { text: [','], kind: PPKind::Punct, .. } if depth == 1 => {
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
    use super::*;

    fn def_test_pair(d: &str, x: &str) -> String {
        let chars = d.chars().collect::<Vec<_>>();
        let mut line = pp_tokenize(&chars);
        line[0].ws = false;
        line[0].nl = false;
        let def = DefineFn::new(&line).unwrap();
        println!("{:?}", &def);

        let chars = x.chars().collect::<Vec<_>>();
        let mut vec = pp_tokenize(&chars);
        def.apply(&mut vec, 0).unwrap();

        format!("{vec:?}")
    }

    #[test]
    fn define() {
        assert_eq!(&def_test_pair("42", "A"), "[{42}]");
        assert_eq!(&def_test_pair("2 + 2", "A"), "[{2}, {+}, {2}]");
        assert_eq!(&def_test_pair("(A, B) B / A", "X(1, 2)"), "[{2}, {/}, {1}]");
        assert_eq!(
            &def_test_pair("(A, B) B / A / B", "X(1, 2)"),
            "[{2}, {/}, {1}, {/}, {2}]"
        );
        assert_eq!(
            &def_test_pair("(A, B) B / A", "X(1, 2 + 2)"),
            "[{2}, {+}, {2}, {/}, {1}]"
        );
        assert_eq!(
            &def_test_pair("(A, B) B / A", "X(1 * 1, 2 + 2)"),
            "[{2}, {+}, {2}, {/}, {1}, {*}, {1}]"
        );
        assert_eq!(
            &def_test_pair("(...) f(0 __VA_OPT__(,) __VA_ARGS__)", "X(1)"),
            "[{f}, {(}, {0}, {,}, {1}, {)}]"
        );
        assert_eq!(
            &def_test_pair(
                "(sname, ...) S sname __VA_OPT__(= { __VA_ARGS__ })",
                "X(xxx, 123)"
            ),
            "[{S}, {xxx}, {=}, {{}, {123}, {}}]"
        );
    }

    #[test]
    fn arg_extraction() {
        const S: &str = r#"(a, 1 + 2, b, f(c), xXx)"#;
        let s = S.chars().collect::<Vec<char>>();
        let tokens = pp_tokenize(&s);
        let (_, args) = extract_args(&tokens).unwrap();

        let result = format!("{args:?}");

        assert_eq!(
            result,
            "[[{a}], [{1}, {+}, {2}], [{b}], [{f}, {(}, {c}, {)}], [{xXx}]]".to_string()
        );
    }
}
