//! Users, roles and their restrictions in the console: `\users`, `\user`,
//! `\roles`, `\role`, `\template`, `\rls` and `\as`.
//!
//! The reading and the expansion live in `open-sdbl-db`; this module is the
//! command layer: it recognizes the line, calls the library, fills the
//! session parameters `\as` learns from the base, and prints the answer.

use std::io::Write;

use open_sdbl::metadata::{MetadataSnapshot, ObjectId};
use open_sdbl::query::{ParameterValue, find_metadata_object};
use open_sdbl_db::access::{
    describe_role, describe_templates, describe_user, ensure_rights, ensure_users,
    list_restrictions, list_roles, list_users, missing_rights, rls_report,
};
use open_sdbl_db::access_cache::{read_current_user, read_template_parameters};
use open_sdbl_db::{AccessStore, DatabaseSession, RestrictionStore, derive_restrictions};

use crate::error::CliError;
use crate::params::ParameterStore;

#[cfg(test)]
#[path = "tests/access.rs"]
mod tests;

const USERS_USAGE: &str = "usage: \\users";
const USER_USAGE: &str = "usage: \\user <имя пользователя>";
const ROLE_USAGE: &str = "usage: \\role <имя роли> [<Вид>.<Объект>]";
const TEMPLATE_USAGE: &str = "usage: \\template <имя роли> [<имя шаблона>]";

/// One console command about users and roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AccessCommand<'line> {
    Users,
    User(&'line str),
    Roles(Option<&'line str>),
    Role {
        name: &'line str,
        object: Option<&'line str>,
    },
    Template {
        role: &'line str,
        name: Option<&'line str>,
    },
    Rls {
        object: &'line str,
        right: Option<&'line str>,
    },
    RlsList,
    As(Option<&'line str>),
    AsClear,
    Usage(&'static str),
}

/// Reads a command line starting with `\users`, `\user`, `\roles`,
/// `\role`, `\rls` or `\as`; other lines are not access commands.
pub(crate) fn parse_access_command(line: &str) -> Option<AccessCommand<'_>> {
    let line = line.trim();
    let (word, rest) = match line.split_once(char::is_whitespace) {
        Some((word, rest)) => (word, rest.trim()),
        None => (line, ""),
    };
    match word.to_lowercase().as_str() {
        "\\users" => Some(if rest.is_empty() {
            AccessCommand::Users
        } else {
            AccessCommand::Usage(USERS_USAGE)
        }),
        "\\user" => Some(if rest.is_empty() {
            AccessCommand::Usage(USER_USAGE)
        } else {
            AccessCommand::User(rest)
        }),
        "\\roles" => Some(AccessCommand::Roles((!rest.is_empty()).then_some(rest))),
        "\\role" => {
            if rest.is_empty() {
                return Some(AccessCommand::Usage(ROLE_USAGE));
            }
            // The object, when given, is the last word with a dot in it.
            let (name, object) = match rest.rsplit_once(char::is_whitespace) {
                Some((name, object)) if object.contains('.') => (name.trim(), Some(object)),
                _ => (rest, None),
            };
            Some(AccessCommand::Role { name, object })
        }
        "\\template" => {
            if rest.is_empty() {
                return Some(AccessCommand::Usage(TEMPLATE_USAGE));
            }
            // The template name, when given, is the last word: a role
            // name may hold spaces, a template name may not.
            let (role, name) = match rest.rsplit_once(char::is_whitespace) {
                Some((role, name)) => (role.trim(), Some(name)),
                None => (rest, None),
            };
            Some(AccessCommand::Template { role, name })
        }
        "\\rls" => {
            if rest.is_empty() {
                return Some(AccessCommand::RlsList);
            }
            let (object, right) = match rest.split_once(char::is_whitespace) {
                Some((object, right)) => (object, Some(right.trim())),
                None => (rest, None),
            };
            Some(AccessCommand::Rls { object, right })
        }
        "\\as" => Some(if rest.is_empty() {
            AccessCommand::As(None)
        } else if rest.eq_ignore_ascii_case("clear") {
            AccessCommand::AsClear
        } else {
            AccessCommand::As(Some(rest))
        }),
        _ => None,
    }
}
/// Runs an access command, reading users and rights when the store lacks
/// them, and prints the answer.
pub(crate) async fn apply_access_command(
    store: &mut AccessStore,
    command: AccessCommand<'_>,
    session: &mut DatabaseSession,
    snapshot: &MetadataSnapshot,
    parameters: &mut ParameterStore,
    restrictions: &mut RestrictionStore,
    output: &mut impl Write,
) -> Result<(), CliError> {
    let session_parameters = &parameters.session_parameters();
    let text = match command {
        AccessCommand::Usage(usage) => return Err(CliError::Data(usage.to_owned())),
        AccessCommand::Users => {
            ensure_users(store, session).await?;
            list_users(store.users().unwrap_or_default(), store.catalog())
        }
        AccessCommand::User(name) => {
            ensure_users(store, session).await?;
            let user = store
                .user(name)
                .ok_or_else(|| CliError::Data(format!("user {name:?} is not in v8users")))?;
            describe_user(user, store.catalog())
        }
        AccessCommand::Roles(filter) => list_roles(store.catalog(), filter),
        AccessCommand::Template { role, name } => {
            let role = store
                .catalog()
                .by_name(role)
                .cloned()
                .ok_or_else(|| CliError::Data(format!("role {role:?} is not declared")))?;
            ensure_rights(store, session, std::slice::from_ref(&role.guid)).await?;
            let rights = store.rights_of(std::slice::from_ref(&role.guid));
            let rights = rights.first().ok_or_else(|| missing_rights(&role.name))?;
            describe_templates(&role, rights, name)?
        }
        AccessCommand::Role { name, object } => {
            let role = store
                .catalog()
                .by_name(name)
                .cloned()
                .ok_or_else(|| CliError::Data(format!("role {name:?} is not declared")))?;
            ensure_rights(store, session, std::slice::from_ref(&role.guid)).await?;
            let rights = store.rights_of(std::slice::from_ref(&role.guid));
            let rights = rights.first().ok_or_else(|| missing_rights(&role.name))?;
            describe_role(&role, rights, snapshot, object)?
        }
        AccessCommand::Rls { object, right } => {
            let roles = match store.current_user() {
                Some(user) => user.data.roles.clone(),
                None => store
                    .catalog()
                    .roles()
                    .iter()
                    .map(|role| role.guid.clone())
                    .collect(),
            };
            ensure_rights(store, session, &roles).await?;
            rls_report(store, snapshot, object, right, session_parameters)?
        }
        AccessCommand::RlsList => list_restrictions(store, snapshot),
        AccessCommand::As(None) => match store.current_user() {
            Some(user) => format!("Current user: {}\n", user.name),
            None => "No current user.\n".to_owned(),
        },
        AccessCommand::AsClear => {
            store.set_current_user(None);
            restrictions.forget_derived();
            "Current user cleared.\n".to_owned()
        }
        AccessCommand::As(Some(name)) => {
            ensure_users(store, session).await?;
            let user = store
                .user(name)
                .cloned()
                .ok_or_else(|| CliError::Data(format!("user {name:?} is not in v8users")))?;
            let unreadable = ensure_rights(store, session, &user.data.roles).await?;
            let roles = user.role_names(store.catalog());
            let user_id = user.data.id.clone();
            store.set_current_user(Some(user));
            let mut text = format!(
                "Current user: {name} ({} roles): {}\n",
                roles.len(),
                roles.join(", ")
            );
            if !unreadable.is_empty() {
                let names = unreadable
                    .iter()
                    .map(|role| store.role_name(role))
                    .collect::<Vec<_>>();
                text.push_str(&format!(
                    "{} roles grant nothing: the base carries no rights resource of {} (a role of an extension, or one the configuration dropped)\n",
                    names.len(),
                    names.join(", ")
                ));
            }
            // The templates read what the base itself carries; what the
            // operator typed stays.
            let read = read_template_parameters(session, snapshot, session_parameters).await?;
            if let Some(reason) = &read.unread {
                text.push_str(&format!("ПараметрыОграниченияДоступа not read: {reason}\n"));
            }
            let mut stored = Vec::new();
            for (name, value) in read.values {
                if parameters.set_if_absent(&name, ParameterValue::String(value)) {
                    stored.push(name);
                }
            }
            // The restrictions compare rows with the element of the user
            // catalog of the information-base user.
            if let Some(current) =
                read_current_user(session, snapshot, session_parameters, &user_id).await?
                && parameters.set_if_absent("ТекущийПользователь", current)
            {
                stored.push("ТекущийПользователь".to_owned());
            }
            // A user of the base is not an external one: the templates
            // compare the parameter with an empty reference.
            if let Ok(object) = find_metadata_object(snapshot, "Справочник.ВнешниеПользователи")
                && parameters.set_if_absent(
                    "ТекущийВнешнийПользователь",
                    ParameterValue::Reference {
                        object: ObjectId::from(&object.guid),
                        id: [0; 16],
                    },
                )
            {
                stored.push("ТекущийВнешнийПользователь".to_owned());
            }
            if !stored.is_empty() {
                stored.sort();
                text.push_str(&format!(
                    "{} session parameters read from the base: {}\n",
                    stored.len(),
                    stored.join(", ")
                ));
            }
            text.push_str(&derive_restrictions(
                store,
                snapshot,
                &parameters.session_parameters(),
                restrictions,
            ));
            text
        }
    };
    output
        .write_all(text.as_bytes())
        .map_err(CliError::standard_output)
}
