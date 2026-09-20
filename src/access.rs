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
    /// The text is malformed at a byte offset, which [`parse_template`]
    /// reports beside the message.
    SyntaxAt {
        /// What is wrong.
        message: String,
        /// The byte offset in the parsed text.
        offset: usize,
    },
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
            Self::SyntaxAt { message, offset } => {
                write!(formatter, "restriction syntax at byte {offset}: {message}")
            }
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
    parse_form(text.trim(), scope.table_name)
}

/// One node of a restriction text or of a template body.
///
/// The offset of every node is a byte offset in the text handed to
/// [`parse_template`], with the comments blanked out.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateNode {
    /// Text kept as it is.
    Text {
        /// The byte offset of the text.
        at: usize,
        /// The text.
        text: String,
    },
    /// `#Если <выражение> #Тогда … #КонецЕсли`, with its branches in
    /// order and the body of `#Иначе`, empty without one.
    Condition {
        /// The byte offset of `#Если`.
        at: usize,
        /// `#Если` and every `#ИначеЕсли`, in order.
        branches: Vec<TemplateBranch>,
        /// The body of `#Иначе`.
        otherwise: Vec<TemplateNode>,
    },
    /// A call `#Имя(аргументы)` of a template the role carries.
    Call {
        /// The byte offset of the `#`.
        at: usize,
        /// The template name, as the text spells it.
        name: String,
        /// The arguments, unquoted as the expansion reads them.
        arguments: Vec<String>,
    },
    /// `#Параметр(N)`, the parameter of the signature by its number.
    Parameter {
        /// The byte offset of the `#`.
        at: usize,
        /// The number as the text writes it, from one.
        number: usize,
    },
    /// Any other `#Имя`: a named parameter of the signature,
    /// `#ИмяТекущейТаблицы`, `#ИмяТекущегоПраваДоступа`.
    Name {
        /// The byte offset of the `#`.
        at: usize,
        /// The name, without the `#`.
        name: String,
    },
}

impl TemplateNode {
    /// The byte offset of the node in the parsed text.
    #[must_use]
    pub fn offset(&self) -> usize {
        match self {
            Self::Text { at, .. }
            | Self::Condition { at, .. }
            | Self::Call { at, .. }
            | Self::Parameter { at, .. }
            | Self::Name { at, .. } => *at,
        }
    }
}

/// One branch of a [`TemplateNode::Condition`].
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateBranch {
    /// The byte offset of `#Если` or `#ИначеЕсли`.
    pub at: usize,
    /// The expression between the directive and `#Тогда`.
    pub condition: String,
    /// The body kept when the expression holds.
    pub body: Vec<TemplateNode>,
}

/// Parses a restriction text, or the body of one template, into its
/// nodes.
///
/// `templates` are the templates of the role: a `#Имя(…)` is read as a
/// call only when the role carries a template of that name, as the
/// expansion reads it. Comments are dropped, and every node carries its
/// byte offset in the text.
///
/// # Errors
///
/// Returns [`RestrictionError::SyntaxAt`] when the directives do not
/// balance, and [`RestrictionError::Syntax`] when a call is unbalanced.
pub fn parse_template(
    text: &str,
    templates: &[RestrictionTemplate],
) -> Result<Vec<TemplateNode>, RestrictionError> {
    let blanked = blank_comments(text);
    let pieces = split_directives(&blanked);
    let mut nodes = Vec::new();
    let mut cursor = 0;
    parse_nodes(&pieces, &mut cursor, &blanked, templates, &mut nodes)?;
    if let Some(piece) = pieces.get(cursor) {
        return Err(RestrictionError::SyntaxAt {
            message: "a closing directive without #Если".to_owned(),
            offset: piece.at(),
        });
    }
    Ok(nodes)
}

/// Parses pieces until a directive closing the enclosing block, which is
/// left for the caller.
fn parse_nodes(
    pieces: &[Piece<'_>],
    cursor: &mut usize,
    text: &str,
    templates: &[RestrictionTemplate],
    out: &mut Vec<TemplateNode>,
) -> Result<(), RestrictionError> {
    while let Some(piece) = pieces.get(*cursor) {
        match piece {
            Piece::Text { at, text: piece } => {
                scan_references(*at, piece, templates, out)?;
                *cursor += 1;
            }
            Piece::Directive {
                at,
                directive: Directive::If,
            } => {
                let at = *at;
                *cursor += 1;
                let node = parse_condition(pieces, cursor, text, templates, at)?;
                out.push(node);
            }
            Piece::Directive { .. } => return Ok(()),
        }
    }
    Ok(())
}

/// Parses an `#Если` block whose `#Если` has been consumed.
fn parse_condition(
    pieces: &[Piece<'_>],
    cursor: &mut usize,
    text: &str,
    templates: &[RestrictionTemplate],
    at: usize,
) -> Result<TemplateNode, RestrictionError> {
    let end = text.len();
    let missing = |message: &str, offset: usize| RestrictionError::SyntaxAt {
        message: message.to_owned(),
        offset,
    };
    let mut branches = Vec::new();
    let mut otherwise = Vec::new();
    let mut branch_at = at;
    loop {
        let condition = match pieces.get(*cursor) {
            Some(Piece::Text { text, .. }) => {
                *cursor += 1;
                (*text).to_owned()
            }
            other => {
                return Err(missing(
                    "#Если without a condition",
                    other.map_or(end, Piece::at),
                ));
            }
        };
        if !is_directive(pieces.get(*cursor), Directive::Then) {
            return Err(missing(
                "#Если without #Тогда",
                pieces.get(*cursor).map_or(end, Piece::at),
            ));
        }
        *cursor += 1;
        let mut body = Vec::new();
        parse_nodes(pieces, cursor, text, templates, &mut body)?;
        branches.push(TemplateBranch {
            at: branch_at,
            condition: condition.trim().to_owned(),
            body,
        });
        match pieces.get(*cursor) {
            Some(Piece::Directive {
                at,
                directive: Directive::ElsIf,
            }) => {
                branch_at = *at;
                *cursor += 1;
            }
            Some(Piece::Directive {
                directive: Directive::Else,
                ..
            }) => {
                *cursor += 1;
                parse_nodes(pieces, cursor, text, templates, &mut otherwise)?;
                if !is_directive(pieces.get(*cursor), Directive::EndIf) {
                    return Err(missing(
                        "#Иначе without #КонецЕсли",
                        pieces.get(*cursor).map_or(end, Piece::at),
                    ));
                }
                *cursor += 1;
                break;
            }
            Some(Piece::Directive {
                directive: Directive::EndIf,
                ..
            }) => {
                *cursor += 1;
                break;
            }
            other => {
                return Err(missing(
                    "#Если without #КонецЕсли",
                    other.map_or(end, Piece::at),
                ));
            }
        }
    }
    Ok(TemplateNode::Condition {
        at,
        branches,
        otherwise,
    })
}

/// Splits one text piece into its calls, parameters, names and the text
/// between them.
fn scan_references(
    base: usize,
    text: &str,
    templates: &[RestrictionTemplate],
    out: &mut Vec<TemplateNode>,
) -> Result<(), RestrictionError> {
    let mut copied = 0;
    let mut from = 0;
    let keep = |out: &mut Vec<TemplateNode>, copied: usize, upto: usize| {
        if upto > copied {
            out.push(TemplateNode::Text {
                at: base + copied,
                text: text[copied..upto].to_owned(),
            });
        }
    };
    while let Some(reference) = next_reference(text, from) {
        if reference.is_escape(text) {
            from = reference.at + 2;
            continue;
        }
        let after_name = reference.after_name().max(reference.at + 1);
        if reference.name.is_empty() {
            from = after_name;
            continue;
        }
        let call = reference.is_call(text);
        let node = if call && names_equal(reference.name, "Параметр") {
            let (arguments, end) = reference
                .arguments(text)
                .ok_or_else(|| RestrictionError::Syntax("unbalanced #Параметр(…)".to_owned()))?;
            let number = arguments
                .first()
                .and_then(|argument| argument.trim().parse::<usize>().ok())
                .unwrap_or(0);
            from = end;
            TemplateNode::Parameter {
                at: base + reference.at,
                number,
            }
        } else if call
            && templates
                .iter()
                .any(|template| names_equal(&template.name, reference.name))
        {
            let (arguments, end) = reference.arguments(text).ok_or_else(|| {
                RestrictionError::Syntax(format!("unbalanced call of #{}", reference.name))
            })?;
            from = end;
            TemplateNode::Call {
                at: base + reference.at,
                name: reference.name.to_owned(),
                arguments,
            }
        } else {
            from = after_name;
            TemplateNode::Name {
                at: base + reference.at,
                name: reference.name.to_owned(),
            }
        };
        keep(out, copied, reference.at);
        out.push(node);
        copied = from;
    }
    keep(out, copied, text.len());
    Ok(())
}

/// Replaces the `//` comments outside string literals by as many bytes of
/// blanks, so the byte offsets of the text are those of the original.
fn blank_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '"' {
            in_string = !in_string;
            out.push(character);
        } else if character == '/' && !in_string && characters.peek() == Some(&'/') {
            out.push_str("  ");
            characters.next();
            for skipped in characters.by_ref() {
                if skipped == '\n' {
                    out.push('\n');
                    break;
                }
                for _ in 0..skipped.len_utf8() {
                    out.push(' ');
                }
            }
        } else {
            out.push(character);
        }
    }
    out
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
    let mut copied = 0;
    let mut from = 0;
    while let Some(reference) = next_reference(text, from) {
        if reference.is_escape(text) {
            from = reference.at + 2;
            continue;
        }
        let after_name = reference.after_name();
        let template = templates
            .iter()
            .find(|template| names_equal(&template.name, reference.name))
            .filter(|_| reference.is_call(text));
        let Some(template) = template else {
            from = after_name.max(reference.at + 1);
            continue;
        };
        if depth >= TEMPLATE_DEPTH {
            return Err(RestrictionError::Syntax(format!(
                "template calls nest deeper than {TEMPLATE_DEPTH} (#{})",
                reference.name
            )));
        }
        let (arguments, end) = reference.arguments(text).ok_or_else(|| {
            RestrictionError::Syntax(format!("unbalanced call of #{}", reference.name))
        })?;
        out.push_str(&text[copied..reference.at]);
        // A body carries its own comments, dropped before its parameters
        // and nested calls are read.
        let body = substitute_parameters(template, &arguments);
        out.push_str(&expand_templates(
            &strip_comments(&body),
            templates,
            depth + 1,
        )?);
        copied = end;
        from = end;
    }
    out.push_str(&text[copied..]);
    Ok(out)
}

/// A `#Имя` written in a text, as both the expansion and the parser read
/// one: the offset of the `#` and the name that follows it.
#[derive(Debug, Clone, Copy)]
struct Reference<'text> {
    at: usize,
    name: &'text str,
}

impl Reference<'_> {
    /// Whether the `#` is the escape `##`, which stands for one `#`.
    fn is_escape(&self, text: &str) -> bool {
        text[self.at + 1..].starts_with('#')
    }

    /// The offset just past the name.
    fn after_name(&self) -> usize {
        self.at + 1 + self.name.len()
    }

    /// Whether the name is written as a call, `#Имя(…)`.
    fn is_call(&self, text: &str) -> bool {
        text[self.after_name()..].trim_start().starts_with('(')
    }

    /// The arguments of the call and the offset just past it.
    fn arguments(&self, text: &str) -> Option<(Vec<String>, usize)> {
        let following = &text[self.after_name()..];
        let open = self.after_name() + following.find('(')?;
        let (arguments, consumed) = call_arguments(&text[open..])?;
        Some((arguments, open + consumed))
    }
}

/// The next `#Имя` at or after `from`.
fn next_reference(text: &str, from: usize) -> Option<Reference<'_>> {
    let at = text[from..].find('#')? + from;
    let after = &text[at + 1..];
    let name_length = after
        .char_indices()
        .find(|(_, character)| !character.is_alphanumeric() && *character != '_')
        .map_or(after.len(), |(index, _)| index);
    Some(Reference {
        at,
        name: &after[..name_length],
    })
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
    const PARAMETER: &str = "#Параметр(";
    let folded = Folded::new(&body);
    let folded_needle = PARAMETER.to_lowercase();
    let mut out = String::with_capacity(body.len());
    let mut copied = 0;
    let mut from = 0;
    while let Some(at) = folded.find(from, PARAMETER, &folded_needle) {
        out.push_str(&body[copied..at]);
        let after = &body[at + PARAMETER.len()..];
        match after.find(')') {
            Some(close) => {
                let number = after[..close].trim().parse::<usize>().unwrap_or(0);
                out.push_str(argument(number.saturating_sub(1)));
                copied = at + PARAMETER.len() + close + 1;
                from = copied;
            }
            None => {
                out.push_str(&body[at..]);
                copied = body.len();
                break;
            }
        }
    }
    out.push_str(&body[copied..]);
    out
}

/// A text prepared for case-insensitive search.
///
/// Lower-casing keeps the byte offsets of Cyrillic and ASCII text, so the
/// lower-cased copy is made once for a whole search instead of once for
/// every occurrence — a template body is expanded thousands of times.
/// When lower-casing does move the offsets the search falls back to the
/// text itself, case-sensitively.
struct Folded<'text> {
    text: &'text str,
    folded: Option<String>,
}

impl<'text> Folded<'text> {
    fn new(text: &'text str) -> Self {
        let folded = text.to_lowercase();
        let folded = (folded.len() == text.len()).then_some(folded);
        Self { text, folded }
    }

    /// The first occurrence of `needle` at or after `from`, where
    /// `folded_needle` is the lower-cased needle.
    fn find(&self, from: usize, needle: &str, folded_needle: &str) -> Option<usize> {
        match &self.folded {
            Some(folded) => folded[from..].find(folded_needle),
            None => self.text[from..].find(needle),
        }
        .map(|at| from + at)
    }
}

/// Whether two names are the same ignoring case, without allocating.
fn names_equal(left: &str, right: &str) -> bool {
    left.chars()
        .flat_map(char::to_lowercase)
        .eq(right.chars().flat_map(char::to_lowercase))
}

fn replace_ignoring_case(text: &str, needle: &str, replacement: &str) -> String {
    let folded = Folded::new(text);
    let folded_needle = needle.to_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut copied = 0;
    let mut from = 0;
    while let Some(at) = folded.find(from, needle, &folded_needle) {
        // The name must end where the parameter ends: `#Поле` is not a
        // prefix of `#ПолеОбъекта`.
        let end = at + needle.len();
        let boundary = text[end..]
            .chars()
            .next()
            .is_none_or(|next| !next.is_alphanumeric() && next != '_');
        out.push_str(&text[copied..at]);
        if boundary {
            out.push_str(replacement);
        } else {
            out.push_str(&text[at..end]);
        }
        copied = end;
        from = end;
    }
    out.push_str(&text[copied..]);
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

/// One piece of a text split at its directives, with the byte offset at
/// which the piece starts.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece<'text> {
    Text { at: usize, text: &'text str },
    Directive { at: usize, directive: Directive },
}

impl Piece<'_> {
    fn at(&self) -> usize {
        match self {
            Self::Text { at, .. } | Self::Directive { at, .. } => *at,
        }
    }
}

fn split_directives(text: &str) -> Vec<Piece<'_>> {
    let mut pieces = Vec::new();
    let mut rest = text;
    let mut text_start = 0;
    let base = text;
    while let Some(hash) = rest.find('#') {
        let after = &rest[hash + 1..];
        // `##` stands for one `#`: neither of them opens a directive.
        if let Some(escaped) = after.strip_prefix('#') {
            rest = escaped;
            continue;
        }
        let name_length = after
            .char_indices()
            .find(|(_, character)| !character.is_alphanumeric())
            .map_or(after.len(), |(index, _)| index);
        match Directive::parse(&after[..name_length]) {
            Some(directive) => {
                let absolute = base.len() - rest.len() + hash;
                if absolute > text_start {
                    pieces.push(Piece::Text {
                        at: text_start,
                        text: &base[text_start..absolute],
                    });
                }
                pieces.push(Piece::Directive {
                    at: absolute,
                    directive,
                });
                rest = &after[name_length..];
                text_start = base.len() - rest.len();
            }
            None => rest = after,
        }
    }
    if text_start < base.len() {
        pieces.push(Piece::Text {
            at: text_start,
            text: &base[text_start..],
        });
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

/// Whether the piece is that directive.
fn is_directive(piece: Option<&Piece<'_>>, wanted: Directive) -> bool {
    matches!(piece, Some(Piece::Directive { directive, .. }) if *directive == wanted)
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
            Piece::Text { text, .. } => {
                if emit {
                    out.push_str(text);
                }
                *cursor += 1;
            }
            Piece::Directive {
                directive: Directive::If,
                ..
            } => {
                *cursor += 1;
                process_if(pieces, cursor, scope, emit, out)?;
            }
            Piece::Directive { .. } => return Ok(()),
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
            Some(Piece::Text { text, .. }) => {
                *cursor += 1;
                *text
            }
            _ => {
                return Err(RestrictionError::Syntax(
                    "#Если without a condition".to_owned(),
                ));
            }
        };
        if !is_directive(pieces.get(*cursor), Directive::Then) {
            return Err(RestrictionError::Syntax("#Если without #Тогда".to_owned()));
        }
        *cursor += 1;
        let holds = emit && !taken && evaluate_expression(condition, scope)?;
        process_pieces(pieces, cursor, scope, holds, out)?;
        taken |= holds;
        match pieces.get(*cursor) {
            Some(Piece::Directive {
                directive: Directive::ElsIf,
                ..
            }) => {
                *cursor += 1;
            }
            Some(Piece::Directive {
                directive: Directive::Else,
                ..
            }) => {
                *cursor += 1;
                process_pieces(pieces, cursor, scope, emit && !taken, out)?;
                if !is_directive(pieces.get(*cursor), Directive::EndIf) {
                    return Err(RestrictionError::Syntax(
                        "#Иначе without #КонецЕсли".to_owned(),
                    ));
                }
                *cursor += 1;
                return Ok(());
            }
            Some(Piece::Directive {
                directive: Directive::EndIf,
                ..
            }) => {
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
                    "имятекущейтаблицы"
                    | "currenttablename"
                    | "текущаятаблица"
                    | "currenttable" => ExprToken::CurrentTable,
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
    // The name of the table stands as a string value where the text reads
    // it by name, and bare where it reads it as a source.
    let quoted = format!("\"{}\"", scope.table_name);
    let text = replace_ignoring_case(text, "#ИмяТекущейТаблицы", &quoted);
    let text = replace_ignoring_case(&text, "#CurrentTableName", &quoted);
    // The name of the table stands for itself where the text reads it as
    // a source, not as a string.
    let text = replace_ignoring_case(&text, &format!("#{CURRENT_TABLE}"), scope.table_name);
    let text = replace_ignoring_case(&text, "#CurrentTable", scope.table_name);
    let text = replace_ignoring_case(
        &text,
        "#ИмяТекущегоПраваДоступа",
        &scope.right.russian_name(),
    );
    let text = replace_ignoring_case(
        &text,
        "#CurrentAccessRightName",
        &scope.right.russian_name(),
    );
    // Last of all, so that no `#` it leaves is read as a directive.
    text.replace("##", "#")
}

/// Reads the platform's form of the expanded text. `table` is the name
/// of the restricted table, which the source description may name.
fn parse_form(text: &str, table: &str) -> Result<ExpandedRestriction, RestrictionError> {
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
        // The full form describes the restricted table after `ИЗ`; the
        // alias it gives is the one the condition reads.
        if next.to_lowercase() == "из" || next.eq_ignore_ascii_case("from") {
            rest = rest[next.len()..].trim_start();
            let source = words.next().unwrap_or_default();
            if source.is_empty() {
                return Err(RestrictionError::Syntax("ИЗ without a source".to_owned()));
            }
            if !names_equal(source, table) && !names_equal(source, CURRENT_TABLE) {
                return Err(RestrictionError::Unsupported(format!(
                    "a restriction reading another table is not supported (ИЗ {source}, restricting {table})"
                )));
            }
            rest = rest[source.len()..].trim_start();
            next = words.next().unwrap_or_default();
            if next.to_lowercase() == "как" || next.eq_ignore_ascii_case("as") {
                rest = rest[next.len()..].trim_start();
                let name = words.next().unwrap_or_default();
                if name.is_empty() {
                    return Err(RestrictionError::Syntax("КАК without an alias".to_owned()));
                }
                alias = Some(name.to_owned());
                rest = rest[name.len()..].trim_start();
                next = words.next().unwrap_or_default();
            }
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
