//! Tokens of tdesktop's `.style` language, as `codegen/common/basic_tokenized_file.cpp`
//! splits them: names may start with digits (`15px`), a run of digits is an
//! integer, `1.5` and `.5` written without spaces are doubles, and comments are
//! dropped.

use anyhow::{bail, ensure};

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Token {
    Name(String),
    Int(i32),
    Double(f64),
    Str(String),
    Punct(char),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Located {
    pub(crate) token: Token,
    pub(crate) line: usize,
}

const PUNCTUATION: &[char] = &[
    ';', ':', ',', '.', '(', ')', '{', '}', '[', ']', '+', '-', '#', '|', '=', '<', '>', '&', '*',
    '/', '%', '!', '?', '~', '^', '@',
];

pub(crate) fn tokenize(source: &str) -> anyhow::Result<Vec<Located>> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut line = 1;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '\n' {
            line += 1;
            i += 1;
        } else if c.is_whitespace() {
            i += 1;
        } else if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            let start = line;
            i += 2;
            loop {
                ensure!(i < chars.len(), "line {start}: unterminated comment");
                if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    i += 2;
                    break;
                }
                if chars[i] == '\n' {
                    line += 1;
                }
                i += 1;
            }
        } else if c == '"' {
            let (value, end) = read_string(&chars, i + 1, line)?;
            tokens.push(Located {
                token: Token::Str(value),
                line,
            });
            i = end;
        } else if is_name_char(c) {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            let digits_end = i;
            while i < chars.len() && is_name_char(chars[i]) {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            let token = if i > digits_end {
                Token::Name(text)
            } else if chars.get(i) == Some(&'.') {
                // `1.` and `1.5` are doubles; whitespace would end the number.
                i += 1;
                let fraction_start = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                let fraction: String = chars[fraction_start..i].iter().collect();
                Token::Double(format!("{text}.{fraction}0").parse()?)
            } else {
                Token::Int(text.parse()?)
            };
            tokens.push(Located { token, line });
        } else if c == '.' && next.is_some_and(|n| n.is_ascii_digit()) {
            let start = i + 1;
            i = start;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            let fraction: String = chars[start..i].iter().collect();
            tokens.push(Located {
                token: Token::Double(format!("0.{fraction}").parse()?),
                line,
            });
        } else if PUNCTUATION.contains(&c) {
            tokens.push(Located {
                token: Token::Punct(c),
                line,
            });
            i += 1;
        } else {
            bail!("line {line}: unexpected character {c:?}");
        }
    }
    Ok(tokens)
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Reads a string literal body starting after the opening quote; returns the
/// value and the index after the closing quote.
fn read_string(chars: &[char], mut i: usize, line: usize) -> anyhow::Result<(String, usize)> {
    let mut value = String::new();
    loop {
        let Some(&c) = chars.get(i) else {
            bail!("line {line}: unterminated string");
        };
        match c {
            '"' => return Ok((value, i + 1)),
            '\n' => bail!("line {line}: unterminated string"),
            '\\' => {
                let escaped = chars.get(i + 1).copied();
                match escaped {
                    Some('n') => value.push('\n'),
                    Some('t') => value.push('\t'),
                    Some('"') => value.push('"'),
                    Some('\\') => value.push('\\'),
                    _ => bail!("line {line}: bad escape in string"),
                }
                i += 2;
            }
            _ => {
                value.push(c);
                i += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(source: &str) -> Vec<Token> {
        tokenize(source)
            .unwrap()
            .into_iter()
            .map(|t| t.token)
            .collect()
    }

    #[test]
    fn splits_values_like_tdesktop() {
        assert_eq!(
            tokens("width: -34px; scale: 0.9; // note\n/* x */ icon {{ \"a\", fg }};"),
            vec![
                Token::Name("width".into()),
                Token::Punct(':'),
                Token::Punct('-'),
                Token::Name("34px".into()),
                Token::Punct(';'),
                Token::Name("scale".into()),
                Token::Punct(':'),
                Token::Double(0.9),
                Token::Punct(';'),
                Token::Name("icon".into()),
                Token::Punct('{'),
                Token::Punct('{'),
                Token::Str("a".into()),
                Token::Punct(','),
                Token::Name("fg".into()),
                Token::Punct('}'),
                Token::Punct('}'),
                Token::Punct(';'),
            ]
        );
    }

    #[test]
    fn counts_lines_through_comments() {
        let located = tokenize("/*\n\n*/ a").unwrap();
        assert_eq!(located[0].line, 3);
    }
}
