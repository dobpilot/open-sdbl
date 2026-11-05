//! The fixed client chatter answered without touching the database:
//! `SET`, `RESET`, `SHOW`, transaction control, `DISCARD ALL` and the
//! probes drivers send while connecting. Everything else is SDBL.

/// One recognized command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Chatter {
    /// An empty text, or one holding only comments.
    Empty,
    /// `SET name = value`; `None` for `DEFAULT`.
    Set { name: String, value: Option<String> },
    /// `SET … TRANSACTION … READ WRITE`.
    SetReadWrite,
    /// `RESET name`, or `RESET ALL` with `None`.
    Reset(Option<String>),
    /// `SHOW name`.
    Show(String),
    /// `BEGIN` / `START TRANSACTION`.
    Begin { read_write: bool, tag: &'static str },
    /// `COMMIT` / `END`.
    Commit,
    /// `ROLLBACK` / `ABORT`.
    Rollback,
    /// `DISCARD ALL`.
    DiscardAll,
    /// `SELECT version()`.
    Version,
    /// `SELECT current_schema()`.
    CurrentSchema,
    /// `SELECT 1`.
    SelectOne,
    /// DataGrip's transport-status probe.
    SslStatus,
    /// DataGrip's rounded gateway startup epoch probe.
    StartupTime,
    /// A single string-literal connection probe, with an optional alias.
    SelectText { value: String, name: String },
}

/// Removes SQL and SDBL comments outside quoted text.
fn strip_comments(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    let mut quote: Option<char> = None;
    while let Some(character) = characters.next() {
        if let Some(open) = quote {
            output.push(character);
            if character == open {
                quote = None;
            }
            continue;
        }
        match (character, characters.peek()) {
            ('\'' | '"', _) => {
                quote = Some(character);
                output.push(character);
            }
            ('-', Some('-')) | ('/', Some('/')) => {
                for next in characters.by_ref() {
                    if next == '\n' {
                        output.push('\n');
                        break;
                    }
                }
            }
            ('/', Some('*')) => {
                characters.next();
                let mut previous = ' ';
                for next in characters.by_ref() {
                    if previous == '*' && next == '/' {
                        break;
                    }
                    previous = next;
                }
                output.push(' ');
            }
            _ => output.push(character),
        }
    }
    output
}

/// Splits on semicolons outside quoted text.
fn split_statements(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for character in text.chars() {
        match quote {
            Some(open) => {
                current.push(character);
                if character == open {
                    quote = None;
                }
            }
            None if character == ';' => parts.push(std::mem::take(&mut current)),
            None => {
                if matches!(character, '\'' | '"') {
                    quote = Some(character);
                }
                current.push(character);
            }
        }
    }
    parts.push(current);
    parts
        .into_iter()
        .map(|part| part.trim().to_owned())
        .filter(|part| !part.is_empty())
        .collect()
}

/// The text read as chatter, or `None` when any part of it is SDBL.
pub(crate) fn classify(text: &str) -> Option<Vec<Chatter>> {
    for (sql, probe) in [
        (
            "select ssl from pg_stat_ssl where pid = pg_backend_pid()",
            Chatter::SslStatus,
        ),
        (
            "select round(extract(epoch from pg_postmaster_start_time() at time zone 'UTC')) as startup_time",
            Chatter::StartupTime,
        ),
    ] {
        if crate::catalog::sql::matches_statement(text, sql) {
            return Some(vec![probe]);
        }
    }
    if let Some((value, name)) = crate::catalog::sql::string_probe(text) {
        return Some(vec![Chatter::SelectText { value, name }]);
    }
    let stripped = strip_comments(text);
    let parts = split_statements(&stripped);
    if parts.is_empty() {
        return Some(vec![Chatter::Empty]);
    }
    parts.iter().map(|part| one(part)).collect()
}

fn words(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_lowercase).collect()
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    if let Some(inner) = value
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
    {
        return inner.replace("''", "'");
    }
    if let Some(inner) = value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    {
        return inner.to_owned();
    }
    value.to_owned()
}

fn one(text: &str) -> Option<Chatter> {
    let words = words(text);
    let joined = words.join(" ");
    let compact = joined.replace(' ', "");
    match compact.as_str() {
        "selectversion()" => return Some(Chatter::Version),
        "selectcurrent_schema()" | "selectcurrent_schema" => return Some(Chatter::CurrentSchema),
        "select1" => return Some(Chatter::SelectOne),
        _ => {}
    }
    let first = words.first()?.as_str();
    match first {
        "begin" | "start" => {
            let rest = &words[1..];
            let rest = if first == "start" {
                if rest.first().map(String::as_str) != Some("transaction") {
                    return None;
                }
                &rest[1..]
            } else if matches!(
                rest.first().map(String::as_str),
                Some("transaction" | "work")
            ) {
                &rest[1..]
            } else {
                rest
            };
            let modes = rest.join(" ").replace(',', " ");
            let modes = modes.split_whitespace().collect::<Vec<_>>().join(" ");
            let allowed = [
                "",
                "read only",
                "read write",
                "isolation level read committed",
                "isolation level read committed read only",
                "read only isolation level read committed",
                "isolation level repeatable read",
                "isolation level repeatable read read only",
                "isolation level serializable",
                "isolation level serializable read only",
            ];
            if !allowed.contains(&modes.as_str()) {
                return None;
            }
            Some(Chatter::Begin {
                read_write: modes.contains("read write"),
                tag: if first == "start" {
                    "START TRANSACTION"
                } else {
                    "BEGIN"
                },
            })
        }
        "commit" | "end" => bare_or_work(&words).then_some(Chatter::Commit),
        "rollback" | "abort" => bare_or_work(&words).then_some(Chatter::Rollback),
        "discard" => (joined == "discard all").then_some(Chatter::DiscardAll),
        "show" => {
            if words.len() == 2 {
                Some(Chatter::Show(words[1].clone()))
            } else if joined == "show transaction isolation level" {
                Some(Chatter::Show("transaction_isolation".to_owned()))
            } else {
                None
            }
        }
        "reset" => match words.len() {
            2 if words[1] == "all" => Some(Chatter::Reset(None)),
            2 => Some(Chatter::Reset(Some(words[1].clone()))),
            _ => None,
        },
        "set" => set(text),
        _ => None,
    }
}

/// `COMMIT`, `COMMIT WORK` or `COMMIT TRANSACTION`, and so on.
fn bare_or_work(words: &[String]) -> bool {
    match words {
        [_] => true,
        [_, second] => matches!(second.as_str(), "work" | "transaction"),
        _ => false,
    }
}

fn set(text: &str) -> Option<Chatter> {
    // Keep the value's spelling: only the keywords are case-folded.
    let body = text.trim()[3..].trim_start();
    let lowered = body.to_lowercase();
    let (body, lowered) = ["session ", "local "]
        .iter()
        .find_map(|prefix| {
            lowered
                .strip_prefix(prefix)
                .map(|rest| (&body[body.len() - rest.len()..], rest.to_owned()))
        })
        .unwrap_or((body, lowered.clone()));
    if lowered.starts_with("transaction") || lowered.starts_with("characteristics") {
        let compact = lowered.split_whitespace().collect::<Vec<_>>().join(" ");
        return Some(if compact.contains("read write") {
            Chatter::SetReadWrite
        } else {
            Chatter::Set {
                name: "transaction".to_owned(),
                value: None,
            }
        });
    }
    if let Some(rest) = lowered.strip_prefix("time zone") {
        let value = &body[body.len() - rest.len()..];
        return Some(Chatter::Set {
            name: "timezone".to_owned(),
            value: Some(unquote(value)),
        });
    }
    if lowered == "names" || lowered.starts_with("names ") {
        let value = &body[5.min(body.len())..];
        return Some(Chatter::Set {
            name: "client_encoding".to_owned(),
            value: Some(unquote(value)),
        });
    }
    let split = body.find(|character: char| character == '=' || character.is_whitespace())?;
    let name = body[..split].trim().to_lowercase();
    if name.is_empty()
        || !name
            .chars()
            .all(|character| character.is_alphanumeric() || matches!(character, '_' | '.'))
    {
        return None;
    }
    let rest = body[split..].trim_start();
    let value = if let Some(value) = rest.strip_prefix('=') {
        value
    } else if rest.len() >= 3 && rest[..3].eq_ignore_ascii_case("to ") {
        &rest[3..]
    } else {
        return None;
    };
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some(Chatter::Set {
        name,
        value: (!value.eq_ignore_ascii_case("default")).then(|| unquote(value)),
    })
}

/// Whether an encoding name denotes UTF-8.
pub(crate) fn is_utf8(name: &str) -> bool {
    matches!(
        name.trim()
            .to_ascii_lowercase()
            .replace(['-', '_'], "")
            .as_str(),
        "utf8" | "unicode"
    )
}

#[cfg(test)]
#[path = "tests/chatter.rs"]
mod tests;
