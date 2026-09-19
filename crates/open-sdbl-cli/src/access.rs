//! Users, roles and their restrictions in the console: `\users`, `\user`,
//! `\roles`, `\role`, `\template`, `\rls` and `\as`.
//!
//! The roles come with the metadata snapshot; the users and the rights of
//! roles are read on the first command that needs them, through the same
//! read-only query path as any statement, and kept until `\refresh`. With
//! a current user (`\as`), every restriction target of a `РАЗРЕШЕННЫЕ`
//! batch that `\restrict` does not cover takes the access of the user's
//! roles for `Чтение`, and the console prompt names the user. `\rls`
//! without an object lists the restrictions of the rights already read.

use std::collections::{HashMap, HashSet};
use std::io::Write;

use open_sdbl::access::{Access, RestrictionScope, TemplateNode, parse_template, read_access};
use open_sdbl::metadata::{
    Guid, InfoBaseUser, MetadataSnapshot, MsSqlMetadataQueries, ObjectId, PostgresMetadataQueries,
    RestrictionTemplate, Right, RoleCatalog, RoleEntry, RoleRights, StorageLayout, UserRow,
    parse_role_rights,
};
use open_sdbl::query::{
    AccessRestriction, RestrictionRequest, SessionParameters, find_metadata_object,
    object_query_name,
};

use crate::cells::{Cell, QueryRows};
use crate::error::CliError;
use crate::restrict::RestrictionStore;
use crate::session::{DatabaseDialect, DatabaseSession};

#[cfg(test)]
#[path = "tests/access.rs"]
mod tests;

const USERS_USAGE: &str = "usage: \\users";
const USER_USAGE: &str = "usage: \\user <имя пользователя>";
const ROLE_USAGE: &str = "usage: \\role <имя роли> [<Вид>.<Объект>]";
const TEMPLATE_USAGE: &str = "usage: \\template <имя роли> [<имя шаблона>]";

/// The users, the rights of roles and the current user of the console.
pub(crate) struct AccessStore {
    catalog: RoleCatalog,
    users: Option<Vec<InfoBaseUser>>,
    rights: HashMap<Guid, RoleRights>,
    current: Option<InfoBaseUser>,
}

impl AccessStore {
    /// The store of a snapshot: its roles named, nothing read yet.
    pub(crate) fn new(snapshot: &MetadataSnapshot) -> Self {
        Self {
            catalog: RoleCatalog::from_snapshot(snapshot),
            users: None,
            rights: HashMap::new(),
            current: None,
        }
    }

    /// Forgets what was read: the metadata was reloaded.
    pub(crate) fn reset(&mut self, snapshot: &MetadataSnapshot) {
        *self = Self::new(snapshot);
    }

    pub(crate) fn catalog(&self) -> &RoleCatalog {
        &self.catalog
    }

    pub(crate) fn current_user(&self) -> Option<&InfoBaseUser> {
        self.current.as_ref()
    }

    pub(crate) fn set_users(&mut self, users: Vec<InfoBaseUser>) {
        self.users = Some(users);
    }

    pub(crate) fn users(&self) -> Option<&[InfoBaseUser]> {
        self.users.as_deref()
    }

    pub(crate) fn insert_rights(&mut self, role: Guid, rights: RoleRights) {
        self.rights.insert(role, rights);
    }

    /// The roles among `roles` whose rights are not read yet.
    pub(crate) fn missing_rights(&self, roles: &[Guid]) -> Vec<Guid> {
        roles
            .iter()
            .filter(|role| !self.rights.contains_key(*role))
            .cloned()
            .collect()
    }

    fn user(&self, name: &str) -> Option<&InfoBaseUser> {
        self.users
            .as_deref()?
            .iter()
            .find(|user| user.name.to_lowercase() == name.to_lowercase())
    }

    /// The rights of the roles, in the given order, skipping roles whose
    /// rights are not read.
    fn rights_of(&self, roles: &[Guid]) -> Vec<&RoleRights> {
        roles
            .iter()
            .filter_map(|role| self.rights.get(role))
            .collect()
    }
}

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
    session_parameters: &SessionParameters,
    restrictions: &mut RestrictionStore,
    output: &mut impl Write,
) -> Result<(), CliError> {
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
            describe_templates(&role, rights[0], name)?
        }
        AccessCommand::Role { name, object } => {
            let role = store
                .catalog()
                .by_name(name)
                .cloned()
                .ok_or_else(|| CliError::Data(format!("role {name:?} is not declared")))?;
            ensure_rights(store, session, std::slice::from_ref(&role.guid)).await?;
            let rights = store.rights_of(std::slice::from_ref(&role.guid));
            describe_role(&role, rights[0], snapshot, object)?
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
            store.current = None;
            restrictions.forget_derived();
            "Current user cleared.\n".to_owned()
        }
        AccessCommand::As(Some(name)) => {
            ensure_users(store, session).await?;
            let user = store
                .user(name)
                .cloned()
                .ok_or_else(|| CliError::Data(format!("user {name:?} is not in v8users")))?;
            ensure_rights(store, session, &user.data.roles).await?;
            let roles = user.role_names(store.catalog());
            store.current = Some(user);
            let mut text = format!(
                "Current user: {name} ({} roles): {}\n",
                roles.len(),
                roles.join(", ")
            );
            text.push_str(&derive_restrictions(
                store,
                snapshot,
                session_parameters,
                restrictions,
            ));
            text
        }
    };
    output
        .write_all(text.as_bytes())
        .map_err(CliError::standard_output)
}

async fn ensure_users(
    store: &mut AccessStore,
    session: &mut DatabaseSession,
) -> Result<(), CliError> {
    if store.users().is_some() {
        return Ok(());
    }
    let users = read_users(session).await?;
    store.set_users(users);
    Ok(())
}

/// Reads the rights of the roles the store lacks.
pub(crate) async fn ensure_rights(
    store: &mut AccessStore,
    session: &mut DatabaseSession,
    roles: &[Guid],
) -> Result<(), CliError> {
    let missing = store.missing_rights(roles);
    if missing.is_empty() {
        return Ok(());
    }
    let layout = session.layout().ok_or_else(|| {
        CliError::Data("the storage layout is unknown; run \\refresh first".to_owned())
    })?;
    for (role, rights) in read_role_rights(session, layout, &missing).await? {
        store.insert_rights(role, rights);
    }
    let unread = store.missing_rights(roles);
    if let Some(role) = unread.first() {
        let name = store
            .catalog()
            .by_guid(role)
            .map_or_else(|| role.as_str().to_owned(), |role| role.name.clone());
        return Err(CliError::Data(format!(
            "the rights resource of role {name:?} is not in Config ({} of {} missing)",
            unread.len(),
            roles.len()
        )));
    }
    Ok(())
}

async fn read_users(session: &mut DatabaseSession) -> Result<Vec<InfoBaseUser>, CliError> {
    let (probe, with_email, without_email) = match session.dialect() {
        DatabaseDialect::Postgres => (
            PostgresMetadataQueries::USERS_EMAIL_PROBE,
            PostgresMetadataQueries::USERS_WITH_EMAIL,
            PostgresMetadataQueries::USERS,
        ),
        DatabaseDialect::MsSql { .. } => (
            MsSqlMetadataQueries::USERS_EMAIL_PROBE,
            MsSqlMetadataQueries::USERS_WITH_EMAIL,
            MsSqlMetadataQueries::USERS,
        ),
    };
    // An older platform has no e-mail column: the table is read without it.
    let has_email = session
        .query(probe, 1)
        .await?
        .first()
        .and_then(|row| row.first())
        .is_some_and(|cell| match cell {
            Cell::Number(value) => value.trim() != "0",
            Cell::Bool(value) => *value,
            _ => false,
        });
    let rows = if has_email {
        session.query(with_email, 8).await?
    } else {
        session.query(without_email, 7).await?
    };
    users_from_rows(&rows)
}

/// Decodes the rows of the users statement.
pub(crate) fn users_from_rows(rows: &QueryRows) -> Result<Vec<InfoBaseUser>, CliError> {
    rows.iter()
        .map(|row| {
            let text = |index: usize| match row.get(index) {
                Some(Cell::Text(text)) => Ok(text.as_str()),
                Some(Cell::Null) => Ok(""),
                other => Err(CliError::Data(format!(
                    "v8users column {index} is not text: {other:?}"
                ))),
            };
            let flag = |index: usize| match row.get(index) {
                Some(Cell::Bool(value)) => *value,
                Some(Cell::Number(value)) => value.trim() != "0",
                _ => false,
            };
            let data = match row.get(6) {
                Some(Cell::Bytes(bytes)) => bytes.as_slice(),
                other => {
                    return Err(CliError::Data(format!(
                        "v8users.Data is not bytes: {other:?}"
                    )));
                }
            };
            InfoBaseUser::new(
                UserRow {
                    name: text(0)?,
                    description: text(1)?,
                    os_name: text(2)?,
                    // The eighth column, when the table has it.
                    email: if row.len() > 7 { text(7)? } else { "" },
                    show_in_list: flag(3),
                    standard_authentication: flag(4),
                    administrative: flag(5),
                },
                data,
            )
            .map_err(|error| {
                CliError::Data(format!("v8users {:?}: {error}", text(0).unwrap_or("")))
            })
        })
        .collect()
}

async fn read_role_rights(
    session: &mut DatabaseSession,
    layout: StorageLayout,
    roles: &[Guid],
) -> Result<Vec<(Guid, RoleRights)>, CliError> {
    let statement = match session.dialect() {
        DatabaseDialect::Postgres => PostgresMetadataQueries::role_rights(&layout, roles),
        DatabaseDialect::MsSql { .. } => MsSqlMetadataQueries::role_rights(&layout, roles),
    };
    let rows = session.query(&statement, 3).await?;
    role_rights_from_rows(&rows)
}

/// Assembles the `(file name, part, data)` rows of the rights statement
/// into resources and decodes them.
pub(crate) fn role_rights_from_rows(rows: &QueryRows) -> Result<Vec<(Guid, RoleRights)>, CliError> {
    let mut resources: Vec<(String, Vec<u8>)> = Vec::new();
    for row in rows {
        let name = match row.first() {
            Some(Cell::Text(name)) => name.trim().to_owned(),
            other => {
                return Err(CliError::Data(format!(
                    "Config file name is not text: {other:?}"
                )));
            }
        };
        let bytes = match row.get(2) {
            Some(Cell::Bytes(bytes)) => bytes,
            other => {
                return Err(CliError::Data(format!(
                    "Config data of {name} is not bytes: {other:?}"
                )));
            }
        };
        match resources.last_mut() {
            Some((last, data)) if *last == name => data.extend_from_slice(bytes),
            _ => resources.push((name, bytes.clone())),
        }
    }
    resources
        .into_iter()
        .map(|(name, data)| {
            let guid = name
                .strip_suffix(".0")
                .and_then(|guid| guid.parse::<Guid>().ok())
                .ok_or_else(|| CliError::Data(format!("unexpected Config resource {name}")))?;
            let rights = parse_role_rights(&data)
                .map_err(|error| CliError::Data(format!("rights of role {guid}: {error}")))?;
            Ok((guid, rights))
        })
        .collect()
}

fn flag(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

/// The `\users` listing.
pub(crate) fn list_users(users: &[InfoBaseUser], catalog: &RoleCatalog) -> String {
    let mut text = String::from("name\tdescription\tos login\temail\tshow\tauth\tadmin\troles\n");
    for user in users {
        let named = user
            .data
            .roles
            .iter()
            .filter(|role| catalog.by_guid(role).is_some())
            .count();
        text.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}{}\n",
            user.name,
            user.description,
            user.os_name,
            user.email,
            flag(user.show_in_list),
            flag(user.standard_authentication),
            flag(user.administrative),
            user.data.roles.len(),
            if named == user.data.roles.len() {
                String::new()
            } else {
                format!(" ({} unknown)", user.data.roles.len() - named)
            }
        ));
    }
    text.push_str(&format!("# {} users\n", users.len()));
    text
}

/// The `\user` description.
pub(crate) fn describe_user(user: &InfoBaseUser, catalog: &RoleCatalog) -> String {
    let mut text = format!(
        "name: {}\ndescription: {}\nfull name: {}\nos login: {}\nemail: {}\nid: {}\nshow in list: {}\nstandard authentication: {}\nadministrative: {}\nroles ({}):\n",
        user.name,
        user.description,
        user.data.full_name,
        user.os_name,
        user.email,
        user.data.id,
        flag(user.show_in_list),
        flag(user.standard_authentication),
        flag(user.administrative),
        user.data.roles.len()
    );
    for role in &user.data.roles {
        match catalog.by_guid(role) {
            Some(entry) => text.push_str(&format!(
                "  {}\t{}\n",
                entry.name,
                entry.synonym.as_deref().unwrap_or("")
            )),
            None => text.push_str(&format!("  {role}\t(not in the configuration)\n")),
        }
    }
    text
}

/// The `\roles` listing.
pub(crate) fn list_roles(catalog: &RoleCatalog, filter: Option<&str>) -> String {
    let filter = filter.map(str::to_lowercase);
    let mut text = String::new();
    let mut count = 0;
    for role in catalog.roles() {
        if filter.as_ref().is_some_and(|filter| {
            !role.name.to_lowercase().contains(filter)
                && !role
                    .synonym
                    .as_deref()
                    .is_some_and(|synonym| synonym.to_lowercase().contains(filter))
        }) {
            continue;
        }
        count += 1;
        text.push_str(&format!(
            "{}\t{}\n",
            role.name,
            role.synonym.as_deref().unwrap_or("")
        ));
    }
    text.push_str(&format!("# {count} roles\n"));
    text
}

/// The metadata object of a guid, named as a query names it.
fn object_name(snapshot: &MetadataSnapshot, guid: &Guid) -> String {
    snapshot
        .object_by_id(ObjectId::from(guid))
        .and_then(|object| object_query_name(snapshot, object))
        .unwrap_or_else(|| guid.as_str().to_owned())
}

/// The `\role` description: every object the role grants rights on, or
/// one object with its rights and restriction texts.
pub(crate) fn describe_role(
    role: &RoleEntry,
    rights: &RoleRights,
    snapshot: &MetadataSnapshot,
    object: Option<&str>,
) -> Result<String, CliError> {
    let mut text = format!(
        "role: {}\t{}\nset for new objects: {}\tattributes by default: {}\tindependent child rights: {}\n",
        role.name,
        role.synonym.as_deref().unwrap_or(""),
        flag(rights.set_for_new_objects),
        flag(rights.set_for_attributes_by_default),
        flag(rights.independent_rights_of_child_objects)
    );
    if let Some(name) = object {
        let found = find_metadata_object(snapshot, name)
            .map_err(|error| CliError::Data(error.to_string()))?;
        let Some(entry) = rights.object(&found.guid) else {
            text.push_str(&format!(
                "object: {name}\nnot listed: every right is {} by default\n",
                if rights.set_for_new_objects {
                    "granted"
                } else {
                    "refused"
                }
            ));
            return Ok(text);
        };
        text.push_str(&format!(
            "object: {name}\nrights not listed below are {} by default\n",
            if rights.set_for_new_objects {
                "granted"
            } else {
                "refused"
            }
        ));
        for right in &entry.rights {
            text.push_str(&format!(
                "  {}\t{}\n",
                right.right.russian_name(),
                if right.allowed { "granted" } else { "refused" }
            ));
            for restriction in &right.restrictions {
                text.push_str("    restriction:\n");
                for line in restriction.condition.lines() {
                    text.push_str(&format!("      {}\n", line.trim_end()));
                }
            }
        }
        return Ok(text);
    }
    let mut count = 0;
    for entry in rights
        .objects
        .iter()
        .filter(|entry| entry.members.is_empty())
    {
        let granted = entry
            .rights
            .iter()
            .filter(|right| right.allowed)
            .map(|right| {
                let mut name = right.right.russian_name();
                if !right.restrictions.is_empty() {
                    name.push('*');
                }
                name
            })
            .collect::<Vec<_>>();
        if granted.is_empty() {
            continue;
        }
        count += 1;
        text.push_str(&format!(
            "{}\t{}\n",
            object_name(snapshot, &entry.object),
            granted.join(", ")
        ));
    }
    text.push_str(&format!(
        "# {count} objects; * marks a right with a restriction; {} templates\n",
        rights.templates.len()
    ));
    for template in &rights.templates {
        text.push_str(&format!("  {}\n", signature(template)));
    }
    Ok(text)
}

/// The signature of a template, `ДляРегистра(Регистр, Поле1)`.
fn signature(template: &RestrictionTemplate) -> String {
    format!("{}({})", template.name, template.parameters.join(", "))
}

/// The `\template` report: the templates of a role with the size of each
/// body, or the body of the one named.
pub(crate) fn describe_templates(
    role: &RoleEntry,
    rights: &RoleRights,
    name: Option<&str>,
) -> Result<String, CliError> {
    let Some(name) = name else {
        let mut text = format!("role: {}\n", role.name);
        for template in &rights.templates {
            let parsed = match parse_template(&template.body, &rights.templates) {
                Ok(_) => "ok".to_owned(),
                Err(error) => error.to_string(),
            };
            text.push_str(&format!(
                "{}\t{} bytes\t{parsed}\n",
                signature(template),
                template.body.len()
            ));
        }
        text.push_str(&format!("# {} templates\n", rights.templates.len()));
        return Ok(text);
    };
    let template = rights.template(name).ok_or_else(|| {
        CliError::Data(format!("role {:?} carries no template {name:?}", role.name))
    })?;
    let mut text = format!("role: {}\ntemplate: {}\n", role.name, signature(template));
    match parse_template(&template.body, &rights.templates) {
        Ok(nodes) => text.push_str(&outline(&nodes)),
        Err(error) => text.push_str(&format!("does not parse: {error}\n")),
    }
    for line in template.body.lines() {
        text.push_str(&format!("  {}\n", line.trim_end()));
    }
    Ok(text)
}

/// What a parsed body is made of: the branches, the calls by template
/// name and the names the body reads.
fn outline(nodes: &[TemplateNode]) -> String {
    let mut branches = 0;
    let mut calls: Vec<(String, usize)> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    let mut numbered = 0;
    let mut stack = nodes.iter().collect::<Vec<_>>();
    while let Some(node) = stack.pop() {
        match node {
            TemplateNode::Text { .. } => {}
            TemplateNode::Condition {
                branches: taken,
                otherwise,
                ..
            } => {
                branches += taken.len();
                stack.extend(taken.iter().flat_map(|branch| &branch.body));
                stack.extend(otherwise);
            }
            TemplateNode::Call { name, .. } => {
                match calls.iter_mut().find(|(called, _)| called == name) {
                    Some((_, count)) => *count += 1,
                    None => calls.push((name.clone(), 1)),
                }
            }
            TemplateNode::Parameter { .. } => numbered += 1,
            TemplateNode::Name { name, .. } if !names.iter().any(|known| known == name) => {
                names.push(name.clone());
            }
            _ => {}
        }
    }
    calls.sort();
    names.sort();
    let calls = calls
        .iter()
        .map(|(name, count)| format!("{name}: {count}"))
        .collect::<Vec<_>>();
    let list = |values: Vec<String>| {
        if values.is_empty() {
            "none".to_owned()
        } else {
            values.join(", ")
        }
    };
    format!(
        "branches: {branches}\ncalls: {}\nnumbered parameters: {numbered}\nnames: {}\n",
        list(calls),
        list(names)
    )
}

/// The `\rls` listing: every restriction the rights already read carry,
/// of the current user's roles when one is set. Reads nothing.
pub(crate) fn list_restrictions(store: &AccessStore, snapshot: &MetadataSnapshot) -> String {
    let mut text = match store.current_user() {
        Some(user) => format!("user: {}\n", user.name),
        None => String::new(),
    };
    let mut roles = match store.current_user() {
        Some(user) => user
            .data
            .roles
            .iter()
            .filter_map(|guid| store.rights.get_key_value(guid))
            .collect::<Vec<_>>(),
        None => store.rights.iter().collect(),
    };
    let total = roles.len();
    let name_of = |guid: &Guid| {
        store
            .catalog()
            .by_guid(guid)
            .map_or_else(|| guid.as_str().to_owned(), |role| role.name.clone())
    };
    roles.sort_by_key(|(guid, _)| name_of(guid));
    text.push_str("role\tobject\tright\ttexts\n");
    let mut count = 0;
    let mut restricting = 0;
    for (guid, rights) in roles {
        let role = name_of(guid);
        let mut lines = Vec::new();
        for entry in &rights.objects {
            let object = match entry.members.last() {
                Some(member) => object_name(snapshot, &member.id),
                None => object_name(snapshot, &entry.object),
            };
            for right in &entry.rights {
                if right.restrictions.is_empty() {
                    continue;
                }
                lines.push(format!(
                    "{role}\t{object}\t{}\t{}\n",
                    right.right.russian_name(),
                    right.restrictions.len()
                ));
            }
        }
        if lines.is_empty() {
            continue;
        }
        restricting += 1;
        count += lines.len();
        lines.sort();
        text.extend(lines);
    }
    if total == 0 {
        text.push_str("# no rights read yet; \\role, \\rls <Вид>.<Объект> and \\as read them\n");
    } else {
        text.push_str(&format!(
            "# {count} restrictions in {restricting} of {total} roles read\n"
        ));
    }
    text
}

/// The `\rls` report: the raw restriction of every role that grants the
/// right, and the expanded access.
pub(crate) fn rls_report(
    store: &AccessStore,
    snapshot: &MetadataSnapshot,
    object: &str,
    right: Option<&str>,
    session_parameters: &SessionParameters,
) -> Result<String, CliError> {
    let found = find_metadata_object(snapshot, object)
        .map_err(|error| CliError::Data(error.to_string()))?;
    let right = match right {
        Some(name) => {
            Right::parse(name).ok_or_else(|| CliError::Data(format!("unknown right {name:?}")))?
        }
        None => Right::Read,
    };
    let table_name = object_query_name(snapshot, found).unwrap_or_else(|| object.to_owned());
    let roles = match store.current_user() {
        Some(user) => user.data.roles.clone(),
        None => store
            .catalog()
            .roles()
            .iter()
            .map(|role| role.guid.clone())
            .collect(),
    };
    let mut text = match store.current_user() {
        Some(user) => format!(
            "user: {}\nobject: {table_name}\nright: {}\n",
            user.name,
            right.russian_name()
        ),
        None => format!("object: {table_name}\nright: {}\n", right.russian_name()),
    };
    let mut granting = Vec::new();
    for guid in &roles {
        let Some(rights) = store.rights.get(guid) else {
            continue;
        };
        if !rights.grants(&found.guid, &right) {
            continue;
        }
        let name = store
            .catalog()
            .by_guid(guid)
            .map_or_else(|| guid.as_str().to_owned(), |role| role.name.clone());
        let restrictions = rights.restrictions(&found.guid, &right);
        if restrictions.is_empty() {
            text.push_str(&format!("role {name}: granted without restriction\n"));
        } else {
            for restriction in restrictions {
                text.push_str(&format!("role {name}: restriction\n"));
                for line in restriction.condition.lines() {
                    text.push_str(&format!("    {}\n", line.trim_end()));
                }
            }
        }
        granting.push(rights);
    }
    let scope = RestrictionScope {
        table_name: &table_name,
        right: &right,
        session: session_parameters,
    };
    match read_access(&granting, &found.guid, &right, &scope) {
        Ok(Access::Denied) => text.push_str("access: denied (no role grants the right)\n"),
        Ok(Access::Unrestricted) => text.push_str("access: unrestricted\n"),
        Ok(access @ Access::Restricted(_)) => match access.condition() {
            Ok(Some(condition)) => text.push_str(&format!("access: restricted\n{condition}\n")),
            Ok(None) => text.push_str("access: unrestricted\n"),
            Err(error) => text.push_str(&format!("access: cannot combine: {error}\n")),
        },
        Ok(other) => text.push_str(&format!("access: {other:?}\n")),
        Err(error) => text.push_str(&format!(
            "access: cannot expand: {error}\n(set the session parameters the template reads with \\session)\n"
        )),
    }
    Ok(text)
}

/// Expands the restrictions of the current user into the restriction
/// store and reports what was stored and what could not be expanded.
pub(crate) fn derive_restrictions(
    store: &AccessStore,
    snapshot: &MetadataSnapshot,
    session_parameters: &SessionParameters,
    restrictions: &mut RestrictionStore,
) -> String {
    let (derived, failures) = role_restrictions(store, snapshot, session_parameters);
    let stored = restrictions.derive(derived);
    let mut text = format!("{stored} restrictions derived into \\restrict.\n");
    for (message, count) in failures {
        text.push_str(&format!("  {count} objects not expanded: {message}\n"));
    }
    text
}

/// Collapses the runs of whitespace of a condition outside its string
/// literals, so a restriction expanded from a template body stands on one
/// line in the store and in the listing.
fn one_line(condition: &str) -> String {
    let mut text = String::with_capacity(condition.len());
    let mut in_string = false;
    let mut space = false;
    for character in condition.chars() {
        if character == '"' {
            in_string = !in_string;
        }
        if !in_string && character.is_whitespace() {
            space = !text.is_empty();
            continue;
        }
        if space {
            text.push(' ');
            space = false;
        }
        text.push(character);
    }
    text
}

/// The restrictions of the current user, expanded for storing: one entry
/// per object any of the user's roles restricts on `Чтение` and that the
/// roles together leave restricted, and the distinct expansion errors
/// with the number of objects each covers.
pub(crate) type DerivedRestrictions = (Vec<(String, ObjectId, String)>, Vec<(String, usize)>);

/// Expands every `Чтение` restriction of the current user's roles.
pub(crate) fn role_restrictions(
    store: &AccessStore,
    snapshot: &MetadataSnapshot,
    session_parameters: &SessionParameters,
) -> DerivedRestrictions {
    let Some(user) = store.current_user() else {
        return (Vec::new(), Vec::new());
    };
    let roles = store.rights_of(&user.data.roles);
    // Looking an object up in a role is a scan of everything the role
    // lists, and a configuration has thousands of objects in hundreds of
    // roles: index what each role lists once, and consult on an object
    // only the roles that list it.
    let listed = roles
        .iter()
        .map(|rights| {
            rights
                .objects
                .iter()
                .filter(|entry| entry.members.is_empty())
                .map(|entry| entry.object.as_str())
                .collect::<HashSet<&str>>()
        })
        .collect::<Vec<_>>();
    // Every object a role of the user restricts reading of, once.
    let mut objects = Vec::new();
    for rights in &roles {
        for entry in &rights.objects {
            if !entry.members.is_empty() {
                continue;
            }
            let restricted = entry
                .right(&Right::Read)
                .is_some_and(|right| !right.restrictions.is_empty());
            if restricted && !objects.contains(&entry.object) {
                objects.push(entry.object.clone());
            }
        }
    }
    let mut derived = Vec::new();
    let mut failures: Vec<(String, usize)> = Vec::new();
    for guid in objects {
        // A role that does not list the object grants it by its default:
        // one such role granting leaves the object unrestricted, and the
        // others refuse it and say nothing about the restriction.
        let free = roles
            .iter()
            .zip(&listed)
            .any(|(rights, listed)| rights.set_for_new_objects && !listed.contains(guid.as_str()));
        if free {
            continue;
        }
        let granting = roles
            .iter()
            .zip(&listed)
            .filter(|(_, listed)| listed.contains(guid.as_str()))
            .map(|(rights, _)| *rights)
            .collect::<Vec<_>>();
        if granting.is_empty() {
            continue;
        }
        let id = ObjectId::from(&guid);
        let Some(object) = snapshot.object_by_id(id) else {
            continue;
        };
        let Some(table_name) = object_query_name(snapshot, object) else {
            continue;
        };
        let scope = RestrictionScope {
            table_name: &table_name,
            right: &Right::Read,
            session: session_parameters,
        };
        let condition = match read_access(&granting, &guid, &Right::Read, &scope)
            .and_then(|access| access.condition())
        {
            Ok(Some(condition)) => condition,
            Ok(None) => continue,
            Err(error) => {
                let message = error.to_string();
                match failures.iter_mut().find(|(text, _)| text == &message) {
                    Some((_, count)) => *count += 1,
                    None => failures.push((message, 1)),
                }
                continue;
            }
        };
        derived.push((table_name, id, one_line(&condition)));
    }
    derived.sort_by(|left, right| left.0.cmp(&right.0));
    (derived, failures)
}

/// The restrictions of the current user for the targets of a batch that
/// the manual restrictions do not cover.
pub(crate) fn user_restrictions(
    store: &AccessStore,
    snapshot: &MetadataSnapshot,
    request: &RestrictionRequest,
    covered: &[AccessRestriction],
    session_parameters: &SessionParameters,
) -> Result<Vec<AccessRestriction>, CliError> {
    let Some(user) = store.current_user() else {
        return Ok(Vec::new());
    };
    let roles = store.rights_of(&user.data.roles);
    let mut derived = Vec::new();
    for target in &request.targets {
        if covered.iter().any(|restriction| {
            restriction.object() == target.object
                && match (restriction.section(), target.table_part.as_deref()) {
                    (None, None) => true,
                    (Some(left), Some(right)) => {
                        left.eq_ignore_ascii_case(right)
                            || left.to_lowercase() == right.to_lowercase()
                    }
                    _ => false,
                }
        }) {
            continue;
        }
        let Some(object) = snapshot.object_by_id(target.object) else {
            continue;
        };
        let table_name = object_query_name(snapshot, object)
            .unwrap_or_else(|| object.name.clone().unwrap_or_default());
        let scope = RestrictionScope {
            table_name: &table_name,
            right: &Right::Read,
            session: session_parameters,
        };
        let access = read_access(&roles, &object.guid, &Right::Read, &scope).map_err(|error| {
            CliError::Data(format!(
                "restriction of {table_name} for user {:?}: {error}; set the session parameters the template reads (the session command)",
                user.name
            ))
        })?;
        let Some(condition) = access
            .condition()
            .map_err(|error| CliError::Data(format!("restriction of {table_name}: {error}")))?
        else {
            continue;
        };
        let condition = match (&target.table_part, &access) {
            // A tabular section takes its owner's access through the
            // owner row.
            (Some(_), Access::Restricted(restrictions)) => {
                let alias = restrictions
                    .iter()
                    .find_map(|restriction| restriction.alias.clone())
                    .unwrap_or_else(|| open_sdbl::access::CURRENT_TABLE.to_owned());
                let body = condition
                    .split_once(" ГДЕ ")
                    .map_or(condition.as_str(), |(_, body)| body);
                format!(
                    "Ссылка В (ВЫБРАТЬ {alias}.Ссылка ИЗ {table_name} КАК {alias} ГДЕ ({body}))"
                )
            }
            _ => condition,
        };
        let mut restriction = AccessRestriction::new(target.object, condition);
        if let Some(section) = &target.table_part {
            restriction = restriction.table_part(section.clone());
        }
        derived.push(restriction);
    }
    Ok(derived)
}
