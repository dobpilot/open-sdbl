use super::MetadataError;

/// A value in the brace-serialized format used by 1C configuration resources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// A nested brace-delimited sequence.
    List(Vec<Self>),
    /// A quoted string with doubled quotes already unescaped.
    String(String),
    /// An unquoted scalar such as a number, GUID, or type marker.
    Atom(String),
    /// An omitted value between separators.
    Null,
}

impl Value {
    pub(crate) fn as_list(&self) -> Option<&[Self]> {
        match self {
            Self::List(values) => Some(values),
            _ => None,
        }
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) | Self::Atom(value) => Some(value),
            Self::List(_) | Self::Null => None,
        }
    }

    pub(crate) fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    pub(crate) fn as_u32(&self) -> Option<u32> {
        match self {
            Self::Atom(value) => value.parse().ok(),
            _ => None,
        }
    }
}

/// Parses one complete UTF-8 1C brace-serialized value.
///
/// A UTF-8 BOM is accepted and ignored. Quoted strings use a doubled quote to
/// encode one literal quote.
///
/// # Errors
///
/// Returns a positional [`MetadataError`] for invalid UTF-8, truncated lists,
/// malformed quoting, or trailing input.
pub fn parse_serialized(input: &[u8]) -> Result<Value, MetadataError> {
    let input = input.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(input);
    let text = std::str::from_utf8(input)
        .map_err(|error| MetadataError::at(error.valid_up_to(), "metadata is not valid UTF-8"))?;
    Parser::new(text).parse()
}

struct Parser<'input> {
    input: &'input str,
    offset: usize,
}

impl<'input> Parser<'input> {
    const fn new(input: &'input str) -> Self {
        Self { input, offset: 0 }
    }

    fn parse(mut self) -> Result<Value, MetadataError> {
        self.skip_whitespace();
        if self.offset == self.input.len() {
            return Err(MetadataError::at(0, "empty metadata serialization"));
        }
        let value = self.value()?;
        self.skip_whitespace();
        if self.offset != self.input.len() {
            return Err(MetadataError::at(
                self.offset,
                "unexpected trailing metadata",
            ));
        }
        Ok(value)
    }

    fn value(&mut self) -> Result<Value, MetadataError> {
        match self.current() {
            Some('{') => self.list(),
            Some('"') => self.string(),
            Some(',' | '}') => Ok(Value::Null),
            Some(_) => self.atom(),
            None => Err(MetadataError::at(self.offset, "unexpected end of metadata")),
        }
    }

    fn list(&mut self) -> Result<Value, MetadataError> {
        self.advance();
        self.skip_whitespace();
        let mut values = Vec::new();
        let mut expecting_value = true;

        loop {
            self.skip_whitespace();
            match self.current() {
                None => return Err(MetadataError::at(self.offset, "unterminated metadata list")),
                Some('}') => {
                    if !expecting_value || values.is_empty() {
                        self.advance();
                        return Ok(Value::List(values));
                    }
                    values.push(Value::Null);
                    self.advance();
                    return Ok(Value::List(values));
                }
                Some(',') if expecting_value => {
                    values.push(Value::Null);
                    self.advance();
                }
                Some(',') => {
                    self.advance();
                    expecting_value = true;
                }
                Some(_) if expecting_value => {
                    values.push(self.value()?);
                    expecting_value = false;
                }
                Some(_) => {
                    return Err(MetadataError::at(
                        self.offset,
                        "expected ',' or '}' in metadata list",
                    ));
                }
            }
        }
    }

    fn string(&mut self) -> Result<Value, MetadataError> {
        self.advance();
        let mut value = String::new();
        loop {
            let Some(character) = self.current() else {
                return Err(MetadataError::at(
                    self.offset,
                    "unterminated metadata string",
                ));
            };
            self.advance();
            if character != '"' {
                value.push(character);
                continue;
            }
            if self.current() == Some('"') {
                self.advance();
                value.push('"');
                continue;
            }
            return Ok(Value::String(value));
        }
    }

    fn atom(&mut self) -> Result<Value, MetadataError> {
        let start = self.offset;
        while self
            .current()
            .is_some_and(|character| !matches!(character, ',' | '}' | '{'))
        {
            self.advance();
        }
        let atom = self.input[start..self.offset].trim();
        if atom.is_empty() {
            return Err(MetadataError::at(start, "empty metadata atom"));
        }
        Ok(Value::Atom(atom.to_owned()))
    }

    fn skip_whitespace(&mut self) {
        while self.current().is_some_and(char::is_whitespace) {
            self.advance();
        }
    }

    fn current(&self) -> Option<char> {
        self.input[self.offset..].chars().next()
    }

    fn advance(&mut self) {
        if let Some(character) = self.current() {
            self.offset += character.len_utf8();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Value, parse_serialized};

    #[test]
    fn parses_bom_nested_values_and_doubled_quotes() {
        let value =
            parse_serialized(b"\xef\xbb\xbf{2,{1,0,abc},\"name \"\"quoted\"\"\",{0},,}").unwrap();
        let root = value.as_list().unwrap();
        assert_eq!(root[0], Value::Atom("2".to_owned()));
        assert_eq!(root[2], Value::String("name \"quoted\"".to_owned()));
        assert_eq!(root[4], Value::Null);
        assert_eq!(root[5], Value::Null);
    }

    #[test]
    fn rejects_truncated_and_trailing_values() {
        assert!(parse_serialized(b"{1,{2}").is_err());
        assert!(parse_serialized(b"{1} garbage").is_err());
    }
}
