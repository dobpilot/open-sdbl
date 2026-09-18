//! Access of a user: the expansion of a role's restriction text into a
//! condition, and the combination of the user's roles for one object and
//! right.
//!
//! A restriction text is written in the platform's restriction language
//! with a preprocessor: `#Если <выражение> #Тогда … #ИначеЕсли … #Иначе …
//! #КонецЕсли` keeps one branch by the session parameters, and a template
//! call `#Имя(аргументы)` stands for the body of the role's template of
//! that name with `#Параметр(N)` and the named parameters of its
//! signature replaced by the arguments. The expanded text is the
//! platform's form `[ТекущаяТаблица [КАК <псевдоним>]] [ГДЕ] <условие>`,
//! which the compiler accepts as an [`AccessRestriction`] condition.
//!
//! [`AccessRestriction`]: crate::query::AccessRestriction

use std::fmt;

use crate::metadata::{Guid, RestrictionTemplate, Right, RoleRights};
use crate::query::{ParameterValue, SessionParameters};

/// The names a restriction text is expanded for.
#[derive(Debug, Clone, Copy)]
pub struct RestrictionScope<'scope> {
    /// The name of the restricted table as the query language spells it,
    /// `Справочник.Номенклатура`: the value of `#ИмяТекущейТаблицы`.
    pub table_name: &'scope str,
    /// The right the restriction belongs to: `#ИмяТекущегоПраваДоступа`
    /// is its Russian name.
    pub right: &'scope Right,
    /// The session parameters the directives read.
    pub session: &'scope SessionParameters,
}

/// A restriction text expanded into a condition.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedRestriction {
    /// The alias the text gives the restricted table after `КАК`, if any;
    /// `ТекущаяТаблица` names it in either case.
    pub alias: Option<String>,
    /// The condition after `ГДЕ`.
    pub condition: String,
}

impl ExpandedRestriction {
    /// The restriction in the platform's full form, for the compiler.
    #[must_use]
    pub fn text(&self) -> String {
        match &self.alias {
            Some(alias) => format!("ТекущаяТаблица КАК {alias} ГДЕ {}", self.condition),
            None => format!("ТекущаяТаблица ГДЕ {}", self.condition),
        }
    }
}

/// Why a restriction text could not be expanded.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestrictionError {
    /// A directive reads a session parameter that has no value.
    MissingParameter(String),
    /// The expansion ends in a labelled message the template emits, such
    /// as `Ошибка: Требуется обновить шаблон …`.
    Message(String),
    /// A template call names a template the role does not carry.
    UnknownTemplate(String),
    /// The directives or their expressions are malformed.
    Syntax(String),
    /// The text uses a form the compiler does not support, such as a join
    /// before `ГДЕ`.
    Unsupported(String),
}

impl fmt::Display for RestrictionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingParameter(name) => {
                write!(formatter, "session parameter \"{name}\" has no value")
            }
            Self::Message(message) => formatter.write_str(message),
            Self::UnknownTemplate(name) => {
                write!(formatter, "restriction template \"{name}\" is not defined")
            }
            Self::Syntax(message) => write!(formatter, "restriction syntax: {message}"),
            Self::Unsupported(message) => write!(formatter, "restriction: {message}"),
        }
    }
}

impl std::error::Error for RestrictionError {}

/// The depth of nested template calls the expansion follows.
const TEMPLATE_DEPTH: usize = 8;

/// The qualifier of the restricted table in the restriction language.
pub const CURRENT_TABLE: &str = "ТекущаяТаблица";

/// Expands a restriction text with the templates of its role.
///
/// # Errors
///
/// Returns [`RestrictionError`] when a directive reads a session parameter
/// without a value, the text calls an unknown template, the expansion is
/// a labelled message, the directives are malformed, or the result uses
/// a form the compiler does not support.
pub fn expand_restriction(
    text: &str,
    templates: &[RestrictionTemplate],
    scope: &RestrictionScope<'_>,
) -> Result<ExpandedRestriction, RestrictionError> {
    let text = strip_comments(text);
    let text = expand_templates(&text, templates, 0)?;
    let text = evaluate_directives(&text, scope)?;
    let text = substitute_names(&text, scope);
    parse_form(text.trim())
}

/// Drops `//` comments outside string literals.
fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '"' {
            in_string = !in_string;
            out.push(character);
        } else if character == '/' && !in_string && characters.peek() == Some(&'/') {
            for skipped in characters.by_ref() {
                if skipped == '\n' {
                    out.push('\n');
                    break;
                }
            }
        } else {
            out.push(character);
        }
    }
    out
}

/// Replaces every template call by the template body with its parameters
/// substituted, recursively.
fn expand_templates(
    text: &str,
    templates: &[RestrictionTemplate],
    depth: usize,
) -> Result<String, RestrictionError> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(hash) = rest.find('#') {
        out.push_str(&rest[..hash]);
        let after = &rest[hash + 1..];
        let name_length = after
            .char_indices()
            .find(|(_, character)| !character.is_alphanumeric() && *character != '_')
            .map_or(after.len(), |(index, _)| index);
        let name = &after[..name_length];
        let following = &after[name_length..];
        let template = templates
            .iter()
            .find(|template| template.name.to_lowercase() == name.to_lowercase());
        let Some(template) = template.filter(|_| following.trim_start().starts_with('(')) else {
            out.push('#');
            out.push_str(name);
            rest = following;
            continue;
        };
        if depth >= TEMPLATE_DEPTH {
            return Err(RestrictionError::Syntax(format!(
                "template calls nest deeper than {TEMPLATE_DEPTH} (#{name})"
            )));
        }
        let open = following.find('(').expect("checked the parenthesis");
        let (arguments, consumed) = call_arguments(&following[open..])
            .ok_or_else(|| RestrictionError::Syntax(format!("unbalanced call of #{name}")))?;
        // A body carries its own comments, dropped before its parameters
        // and nested calls are read.
        let body = substitute_parameters(template, &arguments);
        out.push_str(&expand_templates(
            &strip_comments(&body),
            templates,
            depth + 1,
        )?);
        rest = &following[open + consumed..];
    }
    out.push_str(rest);
    Ok(out)
}

/// The arguments of a call whose text starts at `(`, unquoted, with the
/// number of bytes the call spans.
fn call_arguments(text: &str) -> Option<(Vec<String>, usize)> {
    let mut arguments = Vec::new();
    let mut current = String::new();
    let mut depth = 0_usize;
    let mut in_string = false;
    let mut characters = text.char_indices().peekable();
    while let Some((index, character)) = characters.next() {
        match character {
            '"' if in_string && characters.peek().is_some_and(|(_, next)| *next == '"') => {
                current.push('"');
                characters.next();
            }
            '"' => in_string = !in_string,
            '(' if !in_string => {
                depth += 1;
                if depth > 1 {
                    current.push(character);
                }
            }
            ')' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    arguments.push(current.trim().to_owned());
                    return Some((arguments, index + 1));
                }
                current.push(character);
            }
            ',' if !in_string && depth == 1 => {
                arguments.push(current.trim().to_owned());
                current.clear();
            }
            _ => current.push(character),
        }
    }
    None
}

/// Replaces `#Параметр(N)` and the named parameters of the signature by
/// the call arguments; a parameter the call does not supply is empty.
fn substitute_parameters(template: &RestrictionTemplate, arguments: &[String]) -> String {
    let argument = |index: usize| arguments.get(index).map_or("", String::as_str);
    let mut body = template.body.clone();
    // Longer names first, so `#Поле10` is not read as `#Поле1` + `0`.
    let mut named = template.parameters.iter().enumerate().collect::<Vec<_>>();
    named.sort_by_key(|(_, name)| std::cmp::Reverse(name.len()));
    for (index, name) in named {
        body = replace_ignoring_case(&body, &format!("#{name}"), argument(index));
    }
    let mut out = String::with_capacity(body.len());
    let mut rest = body.as_str();
    while let Some(at) = find_ignoring_case(rest, "#Параметр(") {
        out.push_str(&rest[..at]);
        let after = &rest[at + "#Параметр(".len()..];
        match after.find(')') {
            Some(close) => {
                let number = after[..close].trim().parse::<usize>().unwrap_or(0);
                out.push_str(argument(number.saturating_sub(1)));
                rest = &after[close + 1..];
            }
            None => {
                out.push_str(&rest[at..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

fn find_ignoring_case(haystack: &str, needle: &str) -> Option<usize> {
    let lower_haystack = haystack.to_lowercase();
    let lower_needle = needle.to_lowercase();
    // Lower-casing keeps the byte offsets of Cyrillic and ASCII text.
    lower_haystack
        .find(&lower_needle)
        .filter(|_| lower_haystack.len() == haystack.len())
        .or_else(|| haystack.find(needle))
}

fn replace_ignoring_case(text: &str, needle: &str, replacement: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = find_ignoring_case(rest, needle) {
        // The name must end where the parameter ends: `#Поле` is not a
        // prefix of `#ПолеОбъекта`.
        let end = at + needle.len();
        let boundary = rest[end..]
            .chars()
            .next()
            .is_none_or(|next| !next.is_alphanumeric() && next != '_');
        out.push_str(&rest[..at]);
        if boundary {
            out.push_str(replacement);
        } else {
            out.push_str(&rest[at..end]);
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// The directives of the preprocessor, in either language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Directive {
    If,
    ElsIf,
    Then,
    Else,
    EndIf,
}

impl Directive {
    fn parse(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "если" | "if" => Some(Self::If),
            "иначеесли" | "elsif" => Some(Self::ElsIf),
            "тогда" | "then" => Some(Self::Then),
            "иначе" | "else" => Some(Self::Else),
            "конецесли" | "endif" => Some(Self::EndIf),
            _ => None,
        }
    }
}

/// One piece of a text split at its directives.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece<'text> {
    Text(&'text str),
    Directive(Directive),
}

fn split_directives(text: &str) -> Vec<Piece<'_>> {
    let mut pieces = Vec::new();
    let mut rest = text;
    let mut text_start = 0;
    let base = text;
    while let Some(hash) = rest.find('#') {
        let after = &rest[hash + 1..];
        let name_length = after
            .char_indices()
            .find(|(_, character)| !character.is_alphanumeric())
            .map_or(after.len(), |(index, _)| index);
        match Directive::parse(&after[..name_length]) {
            Some(directive) => {
                let absolute = base.len() - rest.len() + hash;
                if absolute > text_start {
                    pieces.push(Piece::Text(&base[text_start..absolute]));
                }
                pieces.push(Piece::Directive(directive));
                rest = &after[name_length..];
                text_start = base.len() - rest.len();
            }
            None => rest = after,
        }
    }
    if text_start < base.len() {
        pieces.push(Piece::Text(&base[text_start..]));
    }
    pieces
}

/// Keeps the branches whose directives hold.
fn evaluate_directives(
    text: &str,
    scope: &RestrictionScope<'_>,
) -> Result<String, RestrictionError> {
    let pieces = split_directives(text);
    let mut out = String::with_capacity(text.len());
    let mut cursor = 0;
    process_pieces(&pieces, &mut cursor, scope, true, &mut out)?;
    if cursor < pieces.len() {
        return Err(RestrictionError::Syntax(
            "#КонецЕсли without #Если".to_owned(),
        ));
    }
    Ok(out)
}

/// Processes pieces until a directive closing the enclosing block, which
/// is left for the caller; `emit` says whether the text is kept.
fn process_pieces(
    pieces: &[Piece<'_>],
    cursor: &mut usize,
    scope: &RestrictionScope<'_>,
    emit: bool,
    out: &mut String,
) -> Result<(), RestrictionError> {
    while let Some(piece) = pieces.get(*cursor) {
        match piece {
            Piece::Text(text) => {
                if emit {
                    out.push_str(text);
                }
                *cursor += 1;
            }
            Piece::Directive(Directive::If) => {
                *cursor += 1;
                process_if(pieces, cursor, scope, emit, out)?;
            }
            Piece::Directive(_) => return Ok(()),
        }
    }
    Ok(())
}

/// Processes an `#Если` block whose `#Если` has been consumed.
fn process_if(
    pieces: &[Piece<'_>],
    cursor: &mut usize,
    scope: &RestrictionScope<'_>,
    emit: bool,
    out: &mut String,
) -> Result<(), RestrictionError> {
    let mut taken = false;
    loop {
        // The condition, then `#Тогда`.
        let condition = match pieces.get(*cursor) {
            Some(Piece::Text(text)) => {
                *cursor += 1;
                *text
            }
            _ => {
                return Err(RestrictionError::Syntax(
                    "#Если without a condition".to_owned(),
                ));
            }
        };
        if pieces.get(*cursor) != Some(&Piece::Directive(Directive::Then)) {
            return Err(RestrictionError::Syntax("#Если without #Тогда".to_owned()));
        }
        *cursor += 1;
        let holds = emit && !taken && evaluate_expression(condition, scope)?;
        process_pieces(pieces, cursor, scope, holds, out)?;
        taken |= holds;
        match pieces.get(*cursor) {
            Some(Piece::Directive(Directive::ElsIf)) => {
                *cursor += 1;
            }
            Some(Piece::Directive(Directive::Else)) => {
                *cursor += 1;
                process_pieces(pieces, cursor, scope, emit && !taken, out)?;
                if pieces.get(*cursor) != Some(&Piece::Directive(Directive::EndIf)) {
                    return Err(RestrictionError::Syntax(
                        "#Иначе without #КонецЕсли".to_owned(),
                    ));
                }
                *cursor += 1;
                return Ok(());
            }
            Some(Piece::Directive(Directive::EndIf)) => {
                *cursor += 1;
                return Ok(());
            }
            _ => {
                return Err(RestrictionError::Syntax(
                    "#Если without #КонецЕсли".to_owned(),
                ));
            }
        }
    }
}

/// A value of a directive expression.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Val {
    Str(String),
    Bool(bool),
    /// An empty reference: a nil session value or `Значение(….ПустаяСсылка)`.
    EmptyReference,
    /// A reference to a row, by its identifier, or a `Значение(…)` other
    /// than an empty reference, by its text.
    Reference(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExprToken {
    Str(String),
    Parameter(String),
    Word(String),
    CurrentTable,
    CurrentRight,
    Equal,
    NotEqual,
    Plus,
    Open,
    Close,
    Comma,
}

fn tokenize_expression(text: &str) -> Result<Vec<ExprToken>, RestrictionError> {
    let mut tokens = Vec::new();
    let mut characters = text.char_indices().peekable();
    let word_end = |start: usize| {
        text[start..]
            .char_indices()
            .find(|(_, character)| !character.is_alphanumeric() && *character != '_')
            .map_or(text.len(), |(index, _)| start + index)
    };
    while let Some((index, character)) = characters.next() {
        match character {
            character if character.is_whitespace() => {}
            '"' => {
                let mut value = String::new();
                loop {
                    match characters.next() {
                        Some((_, '"')) => {
                            if characters.peek().is_some_and(|(_, next)| *next == '"') {
                                characters.next();
                                value.push('"');
                            } else {
                                break;
                            }
                        }
                        Some((_, other)) => value.push(other),
                        None => {
                            return Err(RestrictionError::Syntax(
                                "unterminated string in a directive".to_owned(),
                            ));
                        }
                    }
                }
                tokens.push(ExprToken::Str(value));
            }
            '&' => {
                let end = word_end(index + 1);
                tokens.push(ExprToken::Parameter(text[index + 1..end].to_owned()));
                while characters.peek().is_some_and(|(next, _)| *next < end) {
                    characters.next();
                }
            }
            '#' => {
                let end = word_end(index + 1);
                let name = text[index + 1..end].to_lowercase();
                tokens.push(match name.as_str() {
                    "имятекущейтаблицы" | "currenttablename" => {
                        ExprToken::CurrentTable
                    }
                    "имятекущегоправадоступа" | "currentaccessrightname" => {
                        ExprToken::CurrentRight
                    }
                    other => {
                        return Err(RestrictionError::Syntax(format!(
                            "unknown directive name #{other} in an expression"
                        )));
                    }
                });
                while characters.peek().is_some_and(|(next, _)| *next < end) {
                    characters.next();
                }
            }
            '=' => tokens.push(ExprToken::Equal),
            '<' if characters.peek().is_some_and(|(_, next)| *next == '>') => {
                characters.next();
                tokens.push(ExprToken::NotEqual);
            }
            '+' => tokens.push(ExprToken::Plus),
            // The dots of a `Значение(Справочник.X.ПустаяСсылка)` path.
            '.' => {}
            '(' => tokens.push(ExprToken::Open),
            ')' => tokens.push(ExprToken::Close),
            ',' => tokens.push(ExprToken::Comma),
            character if character.is_alphanumeric() || character == '_' => {
                let end = word_end(index);
                tokens.push(ExprToken::Word(text[index..end].to_owned()));
                while characters.peek().is_some_and(|(next, _)| *next < end) {
                    characters.next();
                }
            }
            other => {
                return Err(RestrictionError::Syntax(format!(
                    "unexpected {other:?} in a directive expression"
                )));
            }
        }
    }
    Ok(tokens)
}

struct ExprParser<'scope, 'tokens> {
    tokens: &'tokens [ExprToken],
    at: usize,
    scope: &'scope RestrictionScope<'scope>,
}

impl ExprParser<'_, '_> {
    fn peek(&self) -> Option<&ExprToken> {
        self.tokens.get(self.at)
    }

    fn word_is(&self, names: &[&str]) -> bool {
        matches!(self.peek(), Some(ExprToken::Word(word)) if names.iter().any(|name| word.eq_ignore_ascii_case(name) || word.to_lowercase() == *name))
    }

    fn or(&mut self) -> Result<Val, RestrictionError> {
        let mut left = self.and()?;
        while self.word_is(&["или", "or"]) {
            self.at += 1;
            let right = self.and()?;
            left = Val::Bool(boolean(&left, "Или")? || boolean(&right, "Или")?);
        }
        Ok(left)
    }

    fn and(&mut self) -> Result<Val, RestrictionError> {
        let mut left = self.not()?;
        while self.word_is(&["и", "and"]) {
            self.at += 1;
            let right = self.not()?;
            left = Val::Bool(boolean(&left, "И")? && boolean(&right, "И")?);
        }
        Ok(left)
    }

    fn not(&mut self) -> Result<Val, RestrictionError> {
        if self.word_is(&["не", "not"]) {
            self.at += 1;
            let value = self.not()?;
            return Ok(Val::Bool(!boolean(&value, "Не")?));
        }
        self.comparison()
    }

    fn comparison(&mut self) -> Result<Val, RestrictionError> {
        let left = self.concatenation()?;
        let equal = match self.peek() {
            Some(ExprToken::Equal) => true,
            Some(ExprToken::NotEqual) => false,
            _ => return Ok(left),
        };
        self.at += 1;
        let right = self.concatenation()?;
        let same = match (&left, &right) {
            (Val::Str(left), Val::Str(right)) => left == right,
            (Val::Bool(left), Val::Bool(right)) => left == right,
            (Val::EmptyReference, Val::EmptyReference) => true,
            (Val::Reference(left), Val::Reference(right)) => {
                left.to_lowercase() == right.to_lowercase()
            }
            (Val::EmptyReference | Val::Reference(_), Val::EmptyReference | Val::Reference(_)) => {
                false
            }
            _ => {
                return Err(RestrictionError::Syntax(
                    "a directive compares values of different kinds".to_owned(),
                ));
            }
        };
        Ok(Val::Bool(same == equal))
    }

    fn concatenation(&mut self) -> Result<Val, RestrictionError> {
        let mut left = self.primary()?;
        while self.peek() == Some(&ExprToken::Plus) {
            self.at += 1;
            let right = self.primary()?;
            left = Val::Str(format!("{}{}", string(&left)?, string(&right)?));
        }
        Ok(left)
    }

    fn primary(&mut self) -> Result<Val, RestrictionError> {
        let token = self.peek().cloned().ok_or_else(|| {
            RestrictionError::Syntax("directive expression ends early".to_owned())
        })?;
        self.at += 1;
        match token {
            ExprToken::Str(value) => Ok(Val::Str(value)),
            ExprToken::CurrentTable => Ok(Val::Str(self.scope.table_name.to_owned())),
            ExprToken::CurrentRight => Ok(Val::Str(self.scope.right.russian_name())),
            ExprToken::Parameter(name) => parameter_value(self.scope.session, &name),
            ExprToken::Open => {
                let value = self.or()?;
                if self.peek() != Some(&ExprToken::Close) {
                    return Err(RestrictionError::Syntax(
                        "missing closing parenthesis in a directive".to_owned(),
                    ));
                }
                self.at += 1;
                Ok(value)
            }
            ExprToken::Word(word) => match word.to_lowercase().as_str() {
                "истина" | "true" => Ok(Val::Bool(true)),
                "ложь" | "false" => Ok(Val::Bool(false)),
                "неопределено" | "undefined" => Ok(Val::Str("Неопределено".to_owned())),
                // `Значение(Справочник.X.ПустаяСсылка)`: the empty reference;
                // any other value by its spelling.
                "значение" | "value" => {
                    if self.peek() != Some(&ExprToken::Open) {
                        return Err(RestrictionError::Syntax(
                            "Значение without an argument".to_owned(),
                        ));
                    }
                    self.at += 1;
                    let mut path = Vec::new();
                    while let Some(token) = self.peek().cloned() {
                        self.at += 1;
                        match token {
                            ExprToken::Close => break,
                            ExprToken::Word(word) => path.push(word),
                            ExprToken::Str(text) => path.push(text),
                            other => {
                                return Err(RestrictionError::Syntax(format!(
                                    "unexpected {other:?} in Значение"
                                )));
                            }
                        }
                    }
                    let last = path.last().map(|word| word.to_lowercase());
                    Ok(
                        if last.as_deref() == Some("пустаяссылка")
                            || last.as_deref() == Some("emptyref")
                        {
                            Val::EmptyReference
                        } else {
                            Val::Reference(path.join("."))
                        },
                    )
                }
                "стрсодержит" | "strcontains" => {
                    if self.peek() != Some(&ExprToken::Open) {
                        return Err(RestrictionError::Syntax(
                            "СтрСодержит without arguments".to_owned(),
                        ));
                    }
                    self.at += 1;
                    let haystack = self.or()?;
                    if self.peek() != Some(&ExprToken::Comma) {
                        return Err(RestrictionError::Syntax(
                            "СтрСодержит takes two arguments".to_owned(),
                        ));
                    }
                    self.at += 1;
                    let needle = self.or()?;
                    if self.peek() != Some(&ExprToken::Close) {
                        return Err(RestrictionError::Syntax(
                            "СтрСодержит without a closing parenthesis".to_owned(),
                        ));
                    }
                    self.at += 1;
                    Ok(Val::Bool(string(&haystack)?.contains(string(&needle)?)))
                }
                other => Err(RestrictionError::Syntax(format!(
                    "unknown word {other:?} in a directive expression"
                ))),
            },
            other => Err(RestrictionError::Syntax(format!(
                "unexpected {other:?} in a directive expression"
            ))),
        }
    }
}

fn boolean(value: &Val, operator: &str) -> Result<bool, RestrictionError> {
    match value {
        Val::Bool(value) => Ok(*value),
        _ => Err(RestrictionError::Syntax(format!(
            "{operator} applied to a value that is not a boolean in a directive"
        ))),
    }
}

fn string(value: &Val) -> Result<&str, RestrictionError> {
    match value {
        Val::Str(value) => Ok(value),
        _ => Err(RestrictionError::Syntax(
            "a value that is not a string where a string is expected in a directive".to_owned(),
        )),
    }
}

/// The value of a session parameter in a directive: a string, a boolean,
/// or `Неопределено` for `NULL`; anything else has no value here.
fn parameter_value(session: &SessionParameters, name: &str) -> Result<Val, RestrictionError> {
    let parameter = session
        .get(name)
        .ok_or_else(|| RestrictionError::MissingParameter(name.to_owned()))?;
    match parameter.value() {
        ParameterValue::String(value) => Ok(Val::Str(value.clone())),
        ParameterValue::Boolean(value) => Ok(Val::Bool(*value)),
        ParameterValue::Null => Ok(Val::Str("Неопределено".to_owned())),
        ParameterValue::Reference { id, .. } if id.iter().all(|byte| *byte == 0) => {
            Ok(Val::EmptyReference)
        }
        ParameterValue::Reference { id, .. } => Ok(Val::Reference(
            id.iter().map(|byte| format!("{byte:02x}")).collect(),
        )),
        _ => Err(RestrictionError::MissingParameter(name.to_owned())),
    }
}

fn evaluate_expression(text: &str, scope: &RestrictionScope<'_>) -> Result<bool, RestrictionError> {
    let tokens = tokenize_expression(text)?;
    let mut parser = ExprParser {
        tokens: &tokens,
        at: 0,
        scope,
    };
    let value = parser.or()?;
    if parser.at < tokens.len() {
        return Err(RestrictionError::Syntax(
            "unexpected text after a directive expression".to_owned(),
        ));
    }
    boolean(&value, "#Если")
}

/// Replaces the current names in the text kept.
fn substitute_names(text: &str, scope: &RestrictionScope<'_>) -> String {
    let text = replace_ignoring_case(text, "#ИмяТекущейТаблицы", scope.table_name);
    let text = replace_ignoring_case(&text, "#CurrentTableName", scope.table_name);
    let text = replace_ignoring_case(
        &text,
        "#ИмяТекущегоПраваДоступа",
        &scope.right.russian_name(),
    );
    replace_ignoring_case(
        &text,
        "#CurrentAccessRightName",
        &scope.right.russian_name(),
    )
}

/// Reads the platform's form of the expanded text.
fn parse_form(text: &str) -> Result<ExpandedRestriction, RestrictionError> {
    if text.is_empty() {
        return Err(RestrictionError::Syntax(
            "the expansion is empty".to_owned(),
        ));
    }
    // A labelled message: a word right before a colon.
    if let Some((label, _)) = text.split_once(':')
        && !label.is_empty()
        && label
            .chars()
            .all(|character| character.is_alphanumeric() || character == '_')
    {
        return Err(RestrictionError::Message(text.to_owned()));
    }
    let mut words = text.split_whitespace();
    let mut rest = text;
    let mut alias = None;
    let first = words.next().unwrap_or_default();
    if first.eq_ignore_ascii_case(CURRENT_TABLE)
        || first.to_lowercase() == CURRENT_TABLE.to_lowercase()
    {
        rest = rest[first.len()..].trim_start();
        let mut next = words.next().unwrap_or_default();
        if next.eq_ignore_ascii_case("как")
            || next.to_lowercase() == "как"
            || next.eq_ignore_ascii_case("as")
        {
            rest = rest[next.len()..].trim_start();
            let name = words.next().unwrap_or_default();
            if name.is_empty() {
                return Err(RestrictionError::Syntax("КАК without an alias".to_owned()));
            }
            alias = Some(name.to_owned());
            rest = rest[name.len()..].trim_start();
            next = words.next().unwrap_or_default();
        }
        if next.to_lowercase() == "где" || next.eq_ignore_ascii_case("where") {
            rest = rest[next.len()..].trim_start();
        } else if !next.is_empty() {
            return Err(RestrictionError::Unsupported(format!(
                "a restriction joining other tables is not supported (\"{next}\" after {CURRENT_TABLE})"
            )));
        }
    } else if first.to_lowercase() == "где" || first.eq_ignore_ascii_case("where") {
        rest = rest[first.len()..].trim_start();
    }
    if rest.is_empty() {
        return Err(RestrictionError::Syntax(
            "the expansion has no condition".to_owned(),
        ));
    }
    Ok(ExpandedRestriction {
        alias,
        condition: rest.to_owned(),
    })
}

/// The access a set of roles gives to one right of one object.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Access {
    /// No role grants the right.
    Denied,
    /// A role grants the right without a restriction.
    Unrestricted,
    /// Every granting role restricts the right; the rows any of them
    /// allows are accessible.
    Restricted(Vec<ExpandedRestriction>),
}

impl Access {
    /// The restriction condition for the compiler in the platform's full
    /// form: `ТекущаяТаблица ГДЕ ЛОЖЬ` when denied, none when
    /// unrestricted, otherwise the restrictions joined by `ИЛИ`.
    ///
    /// # Errors
    ///
    /// Returns [`RestrictionError::Unsupported`] when the restrictions
    /// give the table different aliases.
    pub fn condition(&self) -> Result<Option<String>, RestrictionError> {
        match self {
            Self::Denied => Ok(Some(format!("{CURRENT_TABLE} ГДЕ ЛОЖЬ"))),
            Self::Unrestricted => Ok(None),
            Self::Restricted(restrictions) => {
                let mut alias: Option<&str> = None;
                for restriction in restrictions {
                    match (&restriction.alias, alias) {
                        (Some(own), Some(seen)) if own.to_lowercase() != seen.to_lowercase() => {
                            return Err(RestrictionError::Unsupported(format!(
                                "the restrictions name the table {seen} and {own}"
                            )));
                        }
                        (Some(own), None) => alias = Some(own),
                        _ => {}
                    }
                }
                let joined = if restrictions.len() == 1 {
                    restrictions[0].condition.clone()
                } else {
                    restrictions
                        .iter()
                        .map(|restriction| format!("({})", restriction.condition))
                        .collect::<Vec<_>>()
                        .join(" ИЛИ ")
                };
                Ok(Some(
                    ExpandedRestriction {
                        alias: alias.map(ToOwned::to_owned),
                        condition: joined,
                    }
                    .text(),
                ))
            }
        }
    }
}

/// Combines the roles of a user for one object and right.
///
/// # Errors
///
/// Returns [`RestrictionError`] when a restriction of a granting role
/// cannot be expanded.
pub fn read_access(
    roles: &[&RoleRights],
    object: &Guid,
    right: &Right,
    scope: &RestrictionScope<'_>,
) -> Result<Access, RestrictionError> {
    let mut restricted = Vec::new();
    let mut granted = false;
    for role in roles {
        if !role.grants(object, right) {
            continue;
        }
        granted = true;
        let restrictions = role.restrictions(object, right);
        if restrictions.is_empty() {
            return Ok(Access::Unrestricted);
        }
        let mut conditions = Vec::with_capacity(restrictions.len());
        let mut alias = None;
        for restriction in restrictions {
            let expanded = expand_restriction(&restriction.condition, &role.templates, scope)?;
            if alias.is_none() {
                alias = expanded.alias;
            }
            conditions.push(expanded.condition);
        }
        // Several restrictions of one right hold together.
        let condition = if conditions.len() == 1 {
            conditions.pop().expect("one condition")
        } else {
            conditions
                .iter()
                .map(|condition| format!("({condition})"))
                .collect::<Vec<_>>()
                .join(" И ")
        };
        restricted.push(ExpandedRestriction { alias, condition });
    }
    Ok(if granted {
        Access::Restricted(restricted)
    } else {
        Access::Denied
    })
}
