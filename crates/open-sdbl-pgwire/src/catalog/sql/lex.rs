//! Bounded PostgreSQL tokenization with character positions.

use crate::codec::ErrorFields;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Kind {
    Word(String),
    Quoted(String),
    String(String),
    Number(String),
    Parameter(usize),
    Symbol(String),
    End,
}

#[derive(Debug, Clone)]
pub(super) struct Token {
    pub(super) kind: Kind,
    pub(super) position: usize,
}

pub(super) fn error(
    code: &'static str,
    message: impl Into<String>,
    position: usize,
) -> ErrorFields {
    let mut error = ErrorFields::error(code, message);
    error.position = Some(position);
    error
}

pub(super) fn lex(text: &str) -> Result<Vec<Token>, ErrorFields> {
    if text.len() > 1_048_576 {
        return Err(error("54001", "catalog SQL exceeds 1 MiB", 1));
    }
    let chars = text.chars().collect::<Vec<_>>();
    let mut result = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let start = i;
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '-' && next == Some('-') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '/' && next == Some('*') {
            i += 2;
            let mut comments = 1;
            while i < chars.len() && comments > 0 {
                match (chars[i], chars.get(i + 1)) {
                    ('/', Some('*')) => {
                        comments += 1;
                        i += 2;
                    }
                    ('*', Some('/')) => {
                        comments -= 1;
                        i += 2;
                    }
                    _ => i += 1,
                }
            }
            if comments != 0 {
                return Err(error("42601", "unterminated comment", start + 1));
            }
            continue;
        }
        let escaped = matches!(c, 'e' | 'E') && next == Some('\'');
        let kind = if c == '\'' || c == '"' || escaped {
            if escaped {
                i += 1;
            }
            let quote = chars[i];
            i += 1;
            let mut value = String::new();
            loop {
                let Some(&c) = chars.get(i) else {
                    return Err(error("42601", "unterminated quoted token", start + 1));
                };
                i += 1;
                if c == quote {
                    if chars.get(i) == Some(&quote) {
                        value.push(c);
                        i += 1;
                    } else {
                        break;
                    }
                } else if escaped && c == '\\' {
                    let Some(&c) = chars.get(i) else {
                        return Err(error("42601", "unterminated escape", start + 1));
                    };
                    i += 1;
                    let decoded = match c {
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        'b' => '\u{0008}',
                        'f' => '\u{000c}',
                        'u' | 'U' | 'x' => {
                            let count = if c == 'u' {
                                4
                            } else if c == 'U' {
                                8
                            } else {
                                2
                            };
                            let mut hex = String::new();
                            while hex.len() < count
                                && chars.get(i).is_some_and(char::is_ascii_hexdigit)
                            {
                                hex.push(chars[i]);
                                i += 1;
                            }
                            if hex.is_empty() || (c != 'x' && hex.len() != count) {
                                return Err(error("22025", "invalid string escape", start + 1));
                            }
                            char::from_u32(
                                u32::from_str_radix(&hex, 16).map_err(|_| {
                                    error("22025", "invalid string escape", start + 1)
                                })?,
                            )
                            .filter(|c| *c != '\0')
                            .ok_or_else(|| error("22021", "invalid Unicode escape", start + 1))?
                        }
                        '0'..='7' => {
                            let mut octal = c.to_string();
                            while octal.len() < 3
                                && chars.get(i).is_some_and(|c| matches!(c, '0'..='7'))
                            {
                                octal.push(chars[i]);
                                i += 1;
                            }
                            let byte = u8::from_str_radix(&octal, 8)
                                .map_err(|_| error("22025", "invalid octal escape", start + 1))?;
                            if !byte.is_ascii() || byte == 0 {
                                return Err(error(
                                    "0A000",
                                    "non-ASCII byte escapes are unsupported",
                                    start + 1,
                                ));
                            }
                            char::from(byte)
                        }
                        c => c,
                    };
                    if c == 'x' && !decoded.is_ascii() {
                        return Err(error(
                            "0A000",
                            "non-ASCII byte escapes are unsupported",
                            start + 1,
                        ));
                    }
                    value.push(decoded);
                } else {
                    value.push(c);
                }
            }
            if value.contains('\0') {
                return Err(error("22021", "NUL in SQL token", start + 1));
            }
            if quote == '"' {
                if value.is_empty() {
                    return Err(error("42601", "empty quoted identifier", start + 1));
                }
                Kind::Quoted(value)
            } else {
                Kind::String(value)
            }
        } else if c == '$' && next.is_some_and(|c| c.is_ascii_digit()) {
            i += 1;
            let begin = i;
            while chars.get(i).is_some_and(char::is_ascii_digit) {
                i += 1;
            }
            let n = chars[begin..i]
                .iter()
                .collect::<String>()
                .parse::<usize>()
                .ok()
                .filter(|n| (1..=65_535).contains(n))
                .ok_or_else(|| error("54001", "invalid parameter number", start + 1))?;
            Kind::Parameter(n)
        } else if c == '$' {
            i += 1;
            while chars
                .get(i)
                .is_some_and(|c| c.is_alphanumeric() || *c == '_')
            {
                i += 1;
            }
            if chars.get(i) != Some(&'$') {
                return Err(error("42601", "invalid dollar quote", start + 1));
            }
            i += 1;
            let delimiter = &chars[start..i];
            let begin = i;
            while i + delimiter.len() <= chars.len() && &chars[i..i + delimiter.len()] != delimiter
            {
                i += 1;
            }
            if i + delimiter.len() > chars.len() {
                return Err(error("42601", "unterminated dollar quote", start + 1));
            }
            let value = chars[begin..i].iter().collect();
            i += delimiter.len();
            Kind::String(value)
        } else if c.is_ascii_digit() {
            i += 1;
            while chars.get(i).is_some_and(char::is_ascii_digit) {
                i += 1;
            }
            if chars.get(i) == Some(&'.') {
                i += 1;
                while chars.get(i).is_some_and(char::is_ascii_digit) {
                    i += 1;
                }
            }
            if chars.get(i) == Some(&'.') {
                return Err(error("42601", "invalid numeric literal", start + 1));
            }
            Kind::Number(chars[start..i].iter().collect())
        } else if c.is_alphabetic() || c == '_' {
            i += 1;
            while chars
                .get(i)
                .is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '$'))
            {
                i += 1;
            }
            Kind::Word(
                chars[start..i]
                    .iter()
                    .collect::<String>()
                    .to_ascii_lowercase(),
            )
        } else {
            i += 1;
            let mut value = c.to_string();
            if let Some(next) = next {
                let pair = format!("{c}{next}");
                if ["::", "<=", ">=", "<>", "!=", "||", "!~", "~*"].contains(&pair.as_str()) {
                    value = pair;
                    i += 1;
                    if value == "!~" && chars.get(i) == Some(&'*') {
                        value.push('*');
                        i += 1;
                    }
                }
            }
            Kind::Symbol(value)
        };
        result.push(Token {
            kind,
            position: start + 1,
        });
    }
    result.push(Token {
        kind: Kind::End,
        position: chars.len() + 1,
    });
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn postgres_tokens_and_failures() {
        let tokens=lex("/* outer /* nested */ */ SELECT \"true\", E'\\x41\\u0411\\n', $tag$literal$tag$, $2::text -- tail").unwrap();
        assert_eq!(tokens[1].kind, Kind::Quoted("true".into()));
        assert_eq!(tokens[3].kind, Kind::String("AБ\n".into()));
        assert!(tokens.iter().any(|t| t.kind == Kind::Parameter(2)));
        for text in ["'open", "/* open", "$tag$open", "\"\"", "1.2.3"] {
            assert_eq!(lex(text).unwrap_err().code, "42601", "{text}");
        }
        assert_eq!(lex("SELECT Ж, '").unwrap_err().position, Some(11));
    }
}
