//! Console query parameters: `\set`, `\params`, `\unset`.
//!
//! Values are parsed from SDBL literals with the library lexer and stored
//! for the session. Before every query the console passes only the
//! parameters the statement references, so stale entries never trigger the
//! compiler's unused-parameter diagnostic.

use open_sdbl::metadata::{MetadataKind, MetadataSnapshot, ObjectId};
use open_sdbl::query::{ParameterDate, ParameterValue, QueryParameter, find_metadata_object};
use open_sdbl::{Keyword, Token, TokenKind, tokenize};

use super::CliError;

const SET_USAGE: &str = "usage: \\set <name> <literal>  (number, \"string\", ИСТИНА/ЛОЖЬ, NULL, ДАТАВРЕМЯ(y, m, d[, h, m, s]), 0x…, ЗНАЧЕНИЕ(Перечисление.X.Y | <Вид>.<Объект>.ПустаяСсылка), or a (list, of, those))";

/// One parameter kept for the console session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredParameter {
    name: String,
    literal: String,
    value: ParameterValue,
}

/// The session parameter store, in insertion order.
#[derive(Debug, Default)]
pub(crate) struct ParameterStore {
    items: Vec<StoredParameter>,
}

impl ParameterStore {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Stores or replaces a parameter, keeping its original position.
    fn set(&mut self, name: &str, literal: &str, value: ParameterValue) {
        let entry = StoredParameter {
            name: name.to_owned(),
            literal: literal.to_owned(),
            value,
        };
        match self
            .items
            .iter_mut()
            .find(|item| names_equal(&item.name, name))
        {
            Some(existing) => *existing = entry,
            None => self.items.push(entry),
        }
    }

    fn unset(&mut self, name: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|item| !names_equal(&item.name, name));
        self.items.len() != before
    }

    /// Stored names, for completion after `&`.
    pub(crate) fn names(&self) -> Vec<String> {
        self.items.iter().map(|item| item.name.clone()).collect()
    }

    /// The parameters a statement references, in store order.
    pub(crate) fn values_for(&self, statement: &str) -> Vec<QueryParameter> {
        let referenced = tokenize(statement)
            .map(|tokens| {
                tokens
                    .into_iter()
                    .filter(|token| token.kind == TokenKind::Parameter)
                    .map(|token| token.lexeme.trim_start_matches('&').to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        self.items
            .iter()
            .filter(|item| referenced.iter().any(|name| names_equal(name, &item.name)))
            .map(|item| QueryParameter::new(item.name.clone(), item.value.clone()))
            .collect()
    }

    /// One line per parameter: name, the literal as entered, the kind.
    pub(crate) fn listing(&self) -> String {
        if self.items.is_empty() {
            return "No parameters set.\n".to_owned();
        }
        let width = self
            .items
            .iter()
            .map(|item| item.name.chars().count())
            .max()
            .unwrap_or(0);
        self.items
            .iter()
            .map(|item| {
                format!(
                    "{:<width$}  {}  [{}]\n",
                    item.name,
                    item.literal,
                    kind_label(&item.value)
                )
            })
            .collect()
    }
}

/// A recognized parameter command line.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ParameterCommand<'line> {
    Set {
        name: &'line str,
        literal: &'line str,
    },
    SetUsage,
    List,
    Unset {
        name: &'line str,
    },
    UnsetUsage,
}

/// Recognizes `\set`, `\params`, and `\unset`; other lines return `None`.
pub(crate) fn parse_parameter_command(line: &str) -> Option<ParameterCommand<'_>> {
    let (command, rest) = line
        .split_once(char::is_whitespace)
        .map_or((line, ""), |(command, rest)| (command, rest.trim()));
    match command {
        "\\params" => Some(ParameterCommand::List),
        "\\set" => {
            let (name, literal) = rest
                .split_once(char::is_whitespace)
                .map_or((rest, ""), |(name, literal)| (name, literal.trim()));
            let name = name.trim_start_matches('&');
            if name.is_empty() || literal.is_empty() || !is_parameter_name(name) {
                return Some(ParameterCommand::SetUsage);
            }
            Some(ParameterCommand::Set { name, literal })
        }
        "\\unset" => {
            let name = rest.trim_start_matches('&');
            if name.is_empty() || !is_parameter_name(name) {
                return Some(ParameterCommand::UnsetUsage);
            }
            Some(ParameterCommand::Unset { name })
        }
        _ => None,
    }
}

/// Applies a parameter command and returns the text to print.
pub(crate) fn apply_parameter_command(
    store: &mut ParameterStore,
    command: ParameterCommand<'_>,
    snapshot: &MetadataSnapshot,
) -> Result<String, CliError> {
    match command {
        ParameterCommand::Set { name, literal } => {
            let value = parse_parameter_literal(literal, snapshot)?;
            let label = kind_label(&value);
            store.set(name, literal, value);
            Ok(format!("Parameter {name} set [{label}].\n"))
        }
        ParameterCommand::SetUsage => Err(CliError::Data(SET_USAGE.to_owned())),
        ParameterCommand::List => Ok(store.listing()),
        ParameterCommand::Unset { name } => {
            if store.unset(name) {
                Ok(format!("Parameter {name} removed.\n"))
            } else {
                Err(CliError::Data(format!("parameter {name:?} is not set")))
            }
        }
        ParameterCommand::UnsetUsage => Err(CliError::Data("usage: \\unset <name>".to_owned())),
    }
}

fn is_parameter_name(name: &str) -> bool {
    let mut characters = name.chars();
    characters
        .next()
        .is_some_and(|first| first == '_' || first.is_alphabetic())
        && characters.all(|character| character == '_' || character.is_alphanumeric())
}

fn names_equal(left: &str, right: &str) -> bool {
    left.chars()
        .flat_map(char::to_uppercase)
        .eq(right.chars().flat_map(char::to_uppercase))
}

/// The kind shown by `\params`.
pub(crate) fn kind_label(value: &ParameterValue) -> String {
    match value {
        ParameterValue::Null => "Null".to_owned(),
        ParameterValue::Boolean(_) => "Boolean".to_owned(),
        ParameterValue::Number { .. } => "Number".to_owned(),
        ParameterValue::String(_) => "String".to_owned(),
        ParameterValue::Date(_) => "DateTime".to_owned(),
        ParameterValue::Reference { .. } => "Reference".to_owned(),
        ParameterValue::Binary(bytes) => format!("Binary[{}]", bytes.len()),
        ParameterValue::List(items) => format!("List[{}]", items.len()),
        _ => "Unknown".to_owned(),
    }
}

/// Parses one SDBL literal into a parameter value.
pub(crate) fn parse_parameter_literal(
    text: &str,
    snapshot: &MetadataSnapshot,
) -> Result<ParameterValue, CliError> {
    let tokens = tokenize(text)
        .map_err(|error| CliError::Data(format!("invalid parameter literal: {error}")))?
        .into_iter()
        .filter(|token| token.kind != TokenKind::Comment)
        .collect::<Vec<_>>();
    let mut parser = LiteralParser {
        tokens: &tokens,
        offset: 0,
        snapshot,
    };
    let value = if parser.peek_lexeme() == Some("(") {
        parser.parse_list()?
    } else {
        parser.parse_scalar()?
    };
    if parser.offset != tokens.len() {
        return Err(CliError::Data(format!(
            "unexpected {:?} after the parameter literal",
            tokens[parser.offset].lexeme
        )));
    }
    Ok(value)
}

struct LiteralParser<'tokens, 'source, 'snapshot> {
    tokens: &'tokens [Token<'source>],
    offset: usize,
    snapshot: &'snapshot MetadataSnapshot,
}

impl<'source> LiteralParser<'_, 'source, '_> {
    fn peek(&self) -> Option<&Token<'source>> {
        self.tokens.get(self.offset)
    }

    fn peek_lexeme(&self) -> Option<&'source str> {
        self.peek().map(|token| token.lexeme)
    }

    fn next(&mut self) -> Result<&Token<'source>, CliError> {
        let token = self
            .tokens
            .get(self.offset)
            .ok_or_else(|| CliError::Data("unexpected end of the parameter literal".to_owned()))?;
        self.offset += 1;
        Ok(token)
    }

    fn expect(&mut self, lexeme: &str) -> Result<(), CliError> {
        let token = self.next()?;
        if token.lexeme == lexeme {
            Ok(())
        } else {
            Err(CliError::Data(format!(
                "expected {lexeme:?} in the parameter literal, found {:?}",
                token.lexeme
            )))
        }
    }

    fn parse_list(&mut self) -> Result<ParameterValue, CliError> {
        self.expect("(")?;
        let mut items = Vec::new();
        if self.peek_lexeme() == Some(")") {
            self.offset += 1;
            return Ok(ParameterValue::List(items));
        }
        loop {
            if self.peek_lexeme() == Some("(") {
                return Err(CliError::Data(
                    "parameter lists cannot contain lists".to_owned(),
                ));
            }
            items.push(self.parse_scalar()?);
            if self.peek_lexeme() == Some(",") {
                self.offset += 1;
                continue;
            }
            self.expect(")")?;
            return Ok(ParameterValue::List(items));
        }
    }

    fn parse_scalar(&mut self) -> Result<ParameterValue, CliError> {
        let negative = self.peek_lexeme() == Some("-");
        if negative {
            self.offset += 1;
        }
        let token = self.next()?;
        let value = match token.kind {
            TokenKind::Number => parse_number(token.lexeme, negative)?,
            _ if negative => {
                return Err(CliError::Data(
                    "a minus sign must be followed by a number".to_owned(),
                ));
            }
            TokenKind::String => ParameterValue::String(
                token
                    .lexeme
                    .strip_prefix('"')
                    .and_then(|inner| inner.strip_suffix('"'))
                    .unwrap_or(token.lexeme)
                    .replace("\"\"", "\""),
            ),
            TokenKind::Binary => ParameterValue::Binary(decode_hex(&token.lexeme[2..])?),
            TokenKind::Keyword(Keyword::True) => ParameterValue::Boolean(true),
            TokenKind::Keyword(Keyword::False) => ParameterValue::Boolean(false),
            TokenKind::Keyword(Keyword::Null) => ParameterValue::Null,
            TokenKind::Keyword(Keyword::DateTime) => self.parse_datetime()?,
            TokenKind::Keyword(Keyword::Value) => self.parse_value()?,
            _ => {
                return Err(CliError::Data(format!(
                    "unsupported parameter literal {:?}; {SET_USAGE}",
                    token.lexeme
                )));
            }
        };
        Ok(value)
    }

    fn parse_datetime(&mut self) -> Result<ParameterValue, CliError> {
        self.expect("(")?;
        let mut components = Vec::new();
        loop {
            let token = self.next()?;
            let component = token.lexeme.parse::<u16>().map_err(|_| {
                CliError::Data(format!(
                    "DATETIME component {:?} must be an integer",
                    token.lexeme
                ))
            })?;
            components.push(component);
            match self.next()?.lexeme {
                "," => continue,
                ")" => break,
                other => {
                    return Err(CliError::Data(format!(
                        "expected \",\" or \")\" in DATETIME, found {other:?}"
                    )));
                }
            }
        }
        if !(3..=6).contains(&components.len()) {
            return Err(CliError::Data(
                "DATETIME requires 3 to 6 integer components".to_owned(),
            ));
        }
        let small = |index: usize| -> Result<u8, CliError> {
            components.get(index).map_or(Ok(0), |value| {
                u8::try_from(*value)
                    .map_err(|_| CliError::Data("DATETIME component is out of range".to_owned()))
            })
        };
        let date = ParameterDate::new(
            components[0],
            small(1)?,
            small(2)?,
            small(3)?,
            small(4)?,
            small(5)?,
        )
        .map_err(|error| CliError::Data(error.to_string()))?;
        Ok(ParameterValue::Date(date))
    }

    fn parse_value(&mut self) -> Result<ParameterValue, CliError> {
        self.expect("(")?;
        let kind = self.next()?.lexeme;
        self.expect(".")?;
        let object = self.next()?.lexeme;
        self.expect(".")?;
        let value = self.next()?.lexeme;
        self.expect(")")?;
        let qualified = format!("{kind}.{object}");
        let metadata_object = find_metadata_object(self.snapshot, &qualified)
            .map_err(|error| CliError::Data(error.message().to_owned()))?;
        let object_id = ObjectId::from(&metadata_object.guid);
        if names_equal(value, "ПустаяСсылка") || names_equal(value, "EmptyRef") {
            if !metadata_object.kind.is_some_and(has_reference_table) {
                return Err(CliError::Data(format!(
                    "{qualified} has no empty reference"
                )));
            }
            return Ok(ParameterValue::Reference {
                object: object_id,
                id: [0; 16],
            });
        }
        if metadata_object.kind != Some(MetadataKind::Enumeration) {
            return Err(CliError::Data(format!(
                "ЗНАЧЕНИЕ({qualified}.{value}) needs a database lookup; write it inline in the query"
            )));
        }
        let predefined = self
            .snapshot
            .predefined_value(object_id, value)
            .map_err(|error| CliError::Data(format!("{qualified}.{value}: {error}")))?;
        Ok(ParameterValue::Reference {
            object: object_id,
            id: predefined.guid.to_1c_bytes(),
        })
    }
}

fn has_reference_table(kind: MetadataKind) -> bool {
    matches!(
        kind,
        MetadataKind::Catalog
            | MetadataKind::Document
            | MetadataKind::Enumeration
            | MetadataKind::ChartOfCharacteristicTypes
            | MetadataKind::ChartOfAccounts
            | MetadataKind::ChartOfCalculationTypes
            | MetadataKind::ExchangePlan
            | MetadataKind::BusinessProcess
            | MetadataKind::Task
    )
}

fn parse_number(lexeme: &str, negative: bool) -> Result<ParameterValue, CliError> {
    let (integer, fraction) = lexeme.split_once('.').unwrap_or((lexeme, ""));
    let digits = format!("{integer}{fraction}");
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(CliError::Data(format!("invalid number literal {lexeme:?}")));
    }
    let unscaled = digits
        .parse::<i128>()
        .map_err(|_| CliError::Data(format!("number {lexeme:?} has too many digits")))?;
    let scale = u8::try_from(fraction.len())
        .map_err(|_| CliError::Data(format!("number {lexeme:?} has too many decimals")))?;
    Ok(ParameterValue::Number {
        unscaled: if negative { -unscaled } else { unscaled },
        scale,
    })
}

fn decode_hex(digits: &str) -> Result<Vec<u8>, CliError> {
    if digits.is_empty() || digits.len() % 2 != 0 {
        return Err(CliError::Data(
            "binary literal needs an even number of hex digits".to_owned(),
        ));
    }
    digits
        .as_bytes()
        .chunks(2)
        .map(|pair| {
            std::str::from_utf8(pair)
                .ok()
                .and_then(|text| u8::from_str_radix(text, 16).ok())
                .ok_or_else(|| CliError::Data("binary literal contains a non-hex digit".to_owned()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        ParameterCommand, ParameterStore, apply_parameter_command, parse_parameter_command,
        parse_parameter_literal,
    };
    use open_sdbl::metadata::{
        ConfigDescriptor, Guid, LiveColumn, LiveTable, MetadataSnapshot, SchemaStorage,
        parse_db_names, resolve_metadata,
    };
    use open_sdbl::query::{ParameterDate, ParameterValue};
    use std::str::FromStr;

    fn enumeration_snapshot() -> MetadataSnapshot {
        let owner = Guid::from_str("c8b21fea-1e3d-4ae9-8719-7ff4db08af97").unwrap();
        let value = Guid::from_str("d2f8bde9-fadd-4be8-9022-249e3a1ac4b9").unwrap();
        let db_names = parse_db_names(&crate::hex_test_support::hex(
            "ab36d4a94eb64832324c4b4dd4354c354ed135494cb5d4b53037b4d4354f4b33494932b0484cb334d75172cd2bcd55d2b1b4acad0500",
        ))
        .unwrap();
        let descriptor =
            |object_guid: Guid, name: &str, enumeration_value: bool| ConfigDescriptor {
                resource_guid: owner.clone(),
                object_guid,
                marker: "1".to_owned(),
                name: name.to_owned(),
                synonyms: Vec::new(),
                comment: None,
                field_purpose: None,
                enumeration_value,
            };
        let status = descriptor(value, "Статус", true);
        let object = descriptor(owner.clone(), "бит_ВидыСтатусовОбъектов", false);
        resolve_metadata(
            db_names,
            vec![object, status],
            SchemaStorage {
                tables: Vec::new(),
                anomalies: Vec::new(),
            },
            vec![LiveTable {
                name: "_enum99".to_owned(),
                columns: vec![
                    LiveColumn {
                        name: "_idrref".to_owned(),
                        data_type: "bytea".to_owned(),
                    },
                    LiveColumn {
                        name: "_enumorder".to_owned(),
                        data_type: "numeric".to_owned(),
                    },
                ],
                indexes: Vec::new(),
            }],
        )
        .snapshot
    }

    #[test]
    fn parses_every_literal_form() {
        let snapshot = enumeration_snapshot();
        let parse = |text: &str| parse_parameter_literal(text, &snapshot).unwrap();
        assert_eq!(
            parse("15.50"),
            ParameterValue::Number {
                unscaled: 1550,
                scale: 2
            }
        );
        assert_eq!(
            parse("-7"),
            ParameterValue::Number {
                unscaled: -7,
                scale: 0
            }
        );
        assert_eq!(
            parse("\"a\"\"b\""),
            ParameterValue::String("a\"b".to_owned())
        );
        assert_eq!(parse("ИСТИНА"), ParameterValue::Boolean(true));
        assert_eq!(parse("false"), ParameterValue::Boolean(false));
        assert_eq!(parse("NULL"), ParameterValue::Null);
        assert_eq!(
            parse("ДАТАВРЕМЯ(2024, 1, 2, 3, 4, 5)"),
            ParameterValue::Date(ParameterDate::new(2024, 1, 2, 3, 4, 5).unwrap())
        );
        assert_eq!(parse("0x0A0b"), ParameterValue::Binary(vec![0x0a, 0x0b]));
        assert!(matches!(
            parse("ЗНАЧЕНИЕ(Перечисление.бит_ВидыСтатусовОбъектов.Статус)"),
            ParameterValue::Reference { .. }
        ));
        assert!(matches!(
            parse("VALUE(Enum.бит_ВидыСтатусовОбъектов.EmptyRef)"),
            ParameterValue::Reference { id, .. } if id == [0; 16]
        ));
        assert_eq!(
            parse("(1, \"x\", NULL)"),
            ParameterValue::List(vec![
                ParameterValue::Number {
                    unscaled: 1,
                    scale: 0
                },
                ParameterValue::String("x".to_owned()),
                ParameterValue::Null,
            ])
        );
        assert_eq!(parse("()"), ParameterValue::List(Vec::new()));
    }

    #[test]
    fn rejects_malformed_literals() {
        let snapshot = enumeration_snapshot();
        for text in [
            "",
            "1 2",
            "ДАТАВРЕМЯ(2024, 13, 1)",
            "ДАТАВРЕМЯ(2024)",
            "((1))",
            "Поле",
            "0x1",
            "ЗНАЧЕНИЕ(Перечисление.бит_ВидыСтатусовОбъектов.Нет)",
            "ЗНАЧЕНИЕ(Справочник.Нет.Значение)",
        ] {
            assert!(
                parse_parameter_literal(text, &snapshot).is_err(),
                "{text:?} should be rejected"
            );
        }
    }

    #[test]
    fn stores_lists_and_filters_parameters_by_reference() {
        let snapshot = enumeration_snapshot();
        let mut store = ParameterStore::new();
        assert_eq!(
            parse_parameter_command("\\set Период ДАТАВРЕМЯ(2024, 1, 1)"),
            Some(ParameterCommand::Set {
                name: "Период",
                literal: "ДАТАВРЕМЯ(2024, 1, 1)"
            })
        );
        assert_eq!(
            parse_parameter_command("\\set &Лимит 10"),
            Some(ParameterCommand::Set {
                name: "Лимит",
                literal: "10"
            })
        );
        assert_eq!(
            parse_parameter_command("\\set"),
            Some(ParameterCommand::SetUsage)
        );
        assert_eq!(
            parse_parameter_command("\\set x"),
            Some(ParameterCommand::SetUsage)
        );
        assert_eq!(
            parse_parameter_command("\\params"),
            Some(ParameterCommand::List)
        );
        assert_eq!(
            parse_parameter_command("\\unset Лимит"),
            Some(ParameterCommand::Unset { name: "Лимит" })
        );
        assert_eq!(parse_parameter_command("\\dt"), None);

        let output = apply_parameter_command(
            &mut store,
            ParameterCommand::Set {
                name: "Период",
                literal: "ДАТАВРЕМЯ(2024, 1, 1)",
            },
            &snapshot,
        )
        .unwrap();
        assert_eq!(output, "Parameter Период set [DateTime].\n");
        apply_parameter_command(
            &mut store,
            ParameterCommand::Set {
                name: "Список",
                literal: "(1, 2)",
            },
            &snapshot,
        )
        .unwrap();
        apply_parameter_command(
            &mut store,
            ParameterCommand::Set {
                name: "период",
                literal: "ДАТАВРЕМЯ(2025, 1, 1)",
            },
            &snapshot,
        )
        .unwrap();
        assert_eq!(
            store.listing(),
            "период  ДАТАВРЕМЯ(2025, 1, 1)  [DateTime]\nСписок  (1, 2)  [List[2]]\n"
        );
        assert_eq!(store.names(), ["период", "Список"]);

        let referenced = store.values_for("ВЫБРАТЬ 1 ГДЕ &ПЕРИОД > 0;");
        assert_eq!(referenced.len(), 1);
        assert_eq!(referenced[0].name(), "период");
        assert!(store.values_for("ВЫБРАТЬ 1;").is_empty());
        assert!(store.values_for("ВЫБРАТЬ \"unterminated").is_empty());

        assert_eq!(
            apply_parameter_command(
                &mut store,
                ParameterCommand::Unset {
                    name: "СПИСОК"
                },
                &snapshot
            )
            .unwrap(),
            "Parameter СПИСОК removed.\n"
        );
        assert!(
            apply_parameter_command(
                &mut store,
                ParameterCommand::Unset {
                    name: "Список"
                },
                &snapshot
            )
            .is_err()
        );
        assert!(
            apply_parameter_command(&mut store, ParameterCommand::SetUsage, &snapshot).is_err()
        );
        assert_eq!(
            apply_parameter_command(&mut store, ParameterCommand::List, &snapshot).unwrap(),
            "период  ДАТАВРЕМЯ(2025, 1, 1)  [DateTime]\n"
        );
    }
}
