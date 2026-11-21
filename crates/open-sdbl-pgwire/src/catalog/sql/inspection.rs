//! Grammar for the composable 1C service calls.

use super::lex::{Kind, Token, lex};
use crate::codec::{ErrorFields, FieldDescription};
use crate::values::{format_of, oid};

const MAX_ARGUMENT_BYTES: usize = 8192;
const MAX_CALLS: usize = 8;
const MAX_PARAMETERS: usize = 16;

#[derive(Clone)]
pub(crate) struct Call {
    pub(crate) function: Function,
    arguments: Vec<Argument>,
}

#[derive(Clone, Copy)]
pub(crate) enum Function {
    Find,
    Show,
    GetObject,
    AttributeText,
    MetadataRef,
    DcsQueries,
}

#[derive(Clone)]
enum Argument {
    Literal(Option<String>),
    Parameter(usize),
    Call(Call),
}

pub(crate) struct BoundCall {
    pub(crate) function: Function,
    pub(crate) arguments: Vec<BoundArgument>,
}

pub(crate) enum BoundArgument {
    Text(Option<String>),
    Call(BoundCall),
}

impl Function {
    pub(crate) fn columns(self) -> &'static [(&'static str, u32)] {
        match self {
            Self::Find => &[("metadata_path", oid::TEXT)],
            Self::Show => &[("onec_show_metadata", oid::TEXT)],
            Self::GetObject => &[("onec_get_object", oid::JSONB)],
            Self::AttributeText => &[("onec_get_attribute_text", oid::TEXT)],
            Self::MetadataRef => &[("onec_get_metadata_ref", oid::TEXT)],
            Self::DcsQueries => &[("name", oid::TEXT), ("query", oid::TEXT)],
        }
    }

    pub(crate) fn fields(self, formats: &[i16]) -> Vec<FieldDescription> {
        self.columns()
            .iter()
            .enumerate()
            .map(|(index, &(name, type_oid))| FieldDescription {
                name: name.into(),
                type_oid,
                type_length: -1,
                type_modifier: -1,
                format: format_of(formats, index),
            })
            .collect()
    }

    fn from_token(token: &Token) -> Option<Self> {
        let (Kind::Word(name) | Kind::Quoted(name)) = &token.kind else {
            return None;
        };
        Some(match name.as_str() {
            "onec_find_metadata" => Self::Find,
            "onec_show_metadata" => Self::Show,
            "onec_get_object" => Self::GetObject,
            "onec_get_attribute_text" => Self::AttributeText,
            "onec_get_metadata_ref" => Self::MetadataRef,
            "onec_dcs_queries" => Self::DcsQueries,
            _ => return None,
        })
    }

    fn scalar_text(self) -> bool {
        matches!(self, Self::Show | Self::AttributeText | Self::MetadataRef)
    }

    fn arity(self) -> usize {
        match self {
            Self::Find | Self::Show | Self::GetObject | Self::DcsQueries => 1,
            Self::AttributeText | Self::MetadataRef => 2,
        }
    }

    pub(crate) fn check_argument(self, value: Option<&str>) -> Result<(), ErrorFields> {
        let invalid = match (self, value) {
            (Self::Show, None | Some("")) => Some("a nonempty metadata path is required"),
            (Self::GetObject, None | Some("")) => Some("a nonempty 1C object link is required"),
            (Self::AttributeText | Self::MetadataRef, Some("")) => {
                Some("a nonempty 1C service argument is required")
            }
            (Self::DcsQueries, Some(xml)) => {
                if xml.len() > crate::inspection::RESOURCE_LIMIT {
                    return Err(ErrorFields::error(
                        "54000",
                        "DCS XML exceeds the gateway limit",
                    ));
                }
                // XML character validation takes place in the parser.
                None
            }
            (_, Some(text)) if text.len() > MAX_ARGUMENT_BYTES || text.contains('\0') => {
                Some(match self {
                    Self::GetObject => "invalid 1C object link",
                    _ => "invalid metadata argument",
                })
            }
            _ => None,
        };
        match invalid {
            Some(message) => Err(ErrorFields::error("22023", message)),
            None => Ok(()),
        }
    }
}

impl Call {
    pub(crate) fn parameter_count(&self) -> usize {
        self.arguments
            .iter()
            .map(|argument| match argument {
                Argument::Literal(_) => 0,
                Argument::Parameter(n) => *n,
                Argument::Call(call) => call.parameter_count(),
            })
            .max()
            .unwrap_or(0)
    }

    pub(crate) fn bind(&self, parameters: &[Option<Vec<u8>>]) -> Result<BoundCall, ErrorFields> {
        let mut arguments = Vec::with_capacity(self.arguments.len());
        for argument in &self.arguments {
            let value = match argument {
                Argument::Literal(value) => value.as_deref(),
                Argument::Parameter(n) => parameters
                    .get(n - 1)
                    .ok_or_else(|| {
                        ErrorFields::error("42P02", format!("parameter ${n} has no value"))
                    })?
                    .as_deref()
                    .map(std::str::from_utf8)
                    .transpose()
                    .map_err(|_| ErrorFields::error("22021", "metadata argument is not UTF-8"))?,
                Argument::Call(call) => {
                    arguments.push(BoundArgument::Call(call.bind(parameters)?));
                    continue;
                }
            };
            self.function.check_argument(value)?;
            arguments.push(BoundArgument::Text(value.map(str::to_owned)));
        }
        Ok(BoundCall {
            function: self.function,
            arguments,
        })
    }
}

pub(crate) fn route(text: &str) -> Option<Result<Call, ErrorFields>> {
    // Avoid a second tokenization for ordinary SDBL and catalog requests.
    if !text
        .as_bytes()
        .windows(5)
        .any(|part| part.eq_ignore_ascii_case(b"onec_"))
    {
        return None;
    }
    let tokens = lex(text).ok()?;
    let [select, tail @ ..] = tokens.as_slice() else {
        return None;
    };
    if !word(select, "select") {
        return None;
    }
    let tail = match tail {
        [star, from, rest @ ..] if symbol(star, "*") && word(from, "from") => rest,
        _ => tail,
    };
    let [name, rest @ ..] = tail else { return None };
    let function = Function::from_token(name)?;
    let mut parser = Parser {
        tokens: rest,
        remaining_calls: MAX_CALLS,
    };
    Some(parser.call(function).and_then(|call| {
        if matches!(parser.tokens, [semi, ..] if symbol(semi, ";")) {
            parser.tokens = &parser.tokens[1..];
        }
        if matches!(
            parser.tokens,
            [Token {
                kind: Kind::End,
                ..
            }]
        ) {
            Ok(call)
        } else {
            Err(syntax(function))
        }
    }))
}

struct Parser<'a> {
    tokens: &'a [Token],
    remaining_calls: usize,
}

impl Parser<'_> {
    fn call(&mut self, function: Function) -> Result<Call, ErrorFields> {
        self.remaining_calls = self
            .remaining_calls
            .checked_sub(1)
            .ok_or_else(|| ErrorFields::error("54000", "too many nested 1C service calls"))?;
        self.take_symbol("(", function)?;
        let mut arguments = Vec::with_capacity(function.arity());
        if matches!(function, Function::Find)
            && matches!(self.tokens, [next, ..] if symbol(next, ")"))
        {
            arguments.push(Argument::Literal(None));
        } else {
            for i in 0..function.arity() {
                if i > 0 {
                    self.take_symbol(",", function)?;
                }
                arguments.push(self.argument(function)?);
            }
        }
        self.take_symbol(")", function)?;
        Ok(Call {
            function,
            arguments,
        })
    }

    fn argument(&mut self, function: Function) -> Result<Argument, ErrorFields> {
        let [token, tail @ ..] = self.tokens else {
            return Err(syntax(function));
        };
        self.tokens = tail;
        match &token.kind {
            Kind::String(value) => Ok(Argument::Literal(Some(value.clone()))),
            Kind::Word(name) if name == "null" => Ok(Argument::Literal(None)),
            Kind::Parameter(n) if *n <= MAX_PARAMETERS => Ok(Argument::Parameter(*n)),
            _ => {
                let child = Function::from_token(token).ok_or_else(|| syntax(function))?;
                if !child.scalar_text() {
                    return Err(ErrorFields::error(
                        "42804",
                        "a nested 1C service function must return one text value",
                    ));
                }
                self.call(child).map(Argument::Call)
            }
        }
    }

    fn take_symbol(&mut self, expected: &str, function: Function) -> Result<(), ErrorFields> {
        let [token, tail @ ..] = self.tokens else {
            return Err(syntax(function));
        };
        if !symbol(token, expected) {
            return Err(syntax(function));
        }
        self.tokens = tail;
        Ok(())
    }
}

fn syntax(function: Function) -> ErrorFields {
    ErrorFields::error(
        "42601",
        match function {
            Function::GetObject => "expected a standalone onec_get_object call",
            Function::Find | Function::Show => {
                "expected a standalone metadata inspection function call"
            }
            _ => "invalid 1C service function call",
        },
    )
}

fn word(token: &Token, value: &str) -> bool {
    matches!(&token.kind, Kind::Word(word) | Kind::Quoted(word) if word == value)
}

fn symbol(token: &Token, value: &str) -> bool {
    matches!(&token.kind, Kind::Symbol(symbol) if symbol == value)
}
