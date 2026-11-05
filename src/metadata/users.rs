//! Users of an information base, as the table `v8users` stores them.
//!
//! The `Data` column is a repeating-key XOR blob: its first byte is the
//! key length, the key follows, and the rest is the brace-serialized user
//! record — with a UTF-8 byte-order mark in front — XORed with the key.
//! The record lists, among other fields, the user identifier, the name,
//! the full name and the block `{N, <guid>…}` of the roles.

use std::fmt;
use std::str::FromStr;

use super::roles::RoleCatalog;
use super::value::{Value, parse_serialized};
use super::{Guid, MetadataError, MetadataErrorKind};
use crate::password::{
    PasswordVerification, SourceProfile, VerificationOptions, verify_stored_password,
};

/// The decoded `Data` of one user: identity, roles and privately retained
/// authentication evidence consumed by [`UserData::verify_password`].
#[non_exhaustive]
#[derive(Clone, PartialEq, Eq)]
pub struct UserData {
    /// The user identifier.
    pub id: Guid,
    /// The user name, as the platform logs it in.
    pub name: String,
    /// The full name.
    pub full_name: String,
    /// The roles, by identifier, in record order.
    pub roles: Vec<Guid>,
    /// Полный список строковых разделителей измеренного формата записи.
    /// `Some` с пустым списком означает известное отсутствие привязок;
    /// `None` означает неподдержанный или неполный формат.
    pub data_separation: Option<Vec<(String, String)>>,
    /// UUID выбранного интерфейсного языка измеренной записи.
    /// `Some` с nil означает отсутствие выбора; `None` — неизвестный формат.
    pub interface_language: Option<Guid>,
    /// The user's public `StoredPasswordValue` (`СохраняемоеЗначениеПароля`)
    /// read from the measured position of the record — the two legacy
    /// digest fields `H1,H2` with the measured algorithm/salt suffix, or a
    /// single extended record string — when
    /// the record carries one. Private and absent from [`fmt::Debug`]: a
    /// stored hash never appears in a public getter, `Debug`, an error or
    /// a log. Only [`UserData::verify_password`] consumes it.
    stored_password: Option<String>,
}

impl fmt::Debug for UserData {
    /// Omits the stored password value entirely; it is a credential.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("UserData")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("full_name", &self.full_name)
            .field("roles", &self.roles)
            .field("data_separation", &self.data_separation)
            .field("interface_language", &self.interface_language)
            .finish_non_exhaustive()
    }
}

impl UserData {
    /// Verifies a password for this user through the `access-rights`
    /// stored-password contract.
    ///
    /// When the record carried a stored password at its measured position
    /// (see [`UserData`]), the candidate is checked against it under the
    /// confirmed [`SourceProfile::PublicStoredValue8327`] source, so a
    /// correct password answers [`VerificationStatus::Match`] and a wrong
    /// one [`VerificationStatus::NoMatch`]. When the record carried none,
    /// the answer is [`VerificationStatus::Indeterminate`] /
    /// [`VerificationReason::SourceProfileUnconfirmed`], so a caller that
    /// authenticates only on [`VerificationStatus::Match`] fails closed.
    ///
    /// The `source` of `options` is overridden either way; the caller's
    /// password state, secondary mode, limits and `record_id` are kept.
    /// Input validation still runs first: an oversized candidate answers
    /// [`VerificationReason::InputTooLarge`].
    ///
    /// [`VerificationStatus::Indeterminate`]: crate::password::VerificationStatus::Indeterminate
    /// [`VerificationStatus::Match`]: crate::password::VerificationStatus::Match
    /// [`VerificationStatus::NoMatch`]: crate::password::VerificationStatus::NoMatch
    /// [`SourceProfile::PublicStoredValue8327`]: crate::password::SourceProfile::PublicStoredValue8327
    /// [`VerificationReason::SourceProfileUnconfirmed`]: crate::password::VerificationReason::SourceProfileUnconfirmed
    /// [`VerificationReason::InputTooLarge`]: crate::password::VerificationReason::InputTooLarge
    #[must_use]
    pub fn verify_password(
        &self,
        candidate: &str,
        options: &VerificationOptions,
    ) -> PasswordVerification {
        match &self.stored_password {
            Some(stored) => {
                let options = options.clone().source(SourceProfile::PublicStoredValue8327);
                verify_stored_password(candidate, stored, &options)
            }
            // A record with no stored value at the measured position is
            // not a confirmed source, so the result stays indeterminate.
            None => {
                let options = options.clone().source(SourceProfile::Unconfirmed);
                verify_stored_password(candidate, "", &options)
            }
        }
    }
}

/// One user of the information base: the row of `v8users` with its
/// decoded data.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoBaseUser {
    /// The user name (`Name`).
    pub name: String,
    /// The description (`Descr`), usually the full name.
    pub description: String,
    /// The operating-system login (`OSName`), empty without one.
    pub os_name: String,
    /// The e-mail (`Email`), empty without one or on a platform whose
    /// table has no such column.
    pub email: String,
    /// Whether the user is shown in the login list (`Show`).
    pub show_in_list: bool,
    /// Whether standard 1C authentication is on (`EAuth`).
    pub standard_authentication: bool,
    /// Whether the user has the administrative rights flag (`AdmRole`).
    pub administrative: bool,
    /// The decoded `Data`.
    pub data: UserData,
}

impl InfoBaseUser {
    /// Combines a row of `v8users` with its decoded data.
    ///
    /// # Errors
    ///
    /// Returns [`MetadataError`] when `data` cannot be decoded.
    pub fn new(row: UserRow<'_>, data: &[u8]) -> Result<Self, MetadataError> {
        Ok(Self {
            name: row.name.to_owned(),
            description: row.description.to_owned(),
            os_name: row.os_name.to_owned(),
            email: row.email.to_owned(),
            show_in_list: row.show_in_list,
            standard_authentication: row.standard_authentication,
            administrative: row.administrative,
            data: decode_user_data(data)?,
        })
    }

    /// Whether the user has any way to log in.
    ///
    /// True when standard 1C authentication is on, or when the
    /// operating-system login is not blank once trimmed. Nothing else
    /// decides it: neither whether the user is shown in the login list,
    /// nor the administrative flag — which is a right, not a way in —
    /// nor the roles, nor the name.
    #[must_use]
    pub fn can_authenticate(&self) -> bool {
        self.standard_authentication || !self.os_name.trim().is_empty()
    }

    /// The names of the user's roles through the catalog; a role the
    /// catalog does not know is named by its identifier.
    #[must_use]
    pub fn role_names(&self, catalog: &RoleCatalog) -> Vec<String> {
        self.data
            .roles
            .iter()
            .map(|guid| {
                catalog
                    .by_guid(guid)
                    .map_or_else(|| guid.as_str().to_owned(), |role| role.name.clone())
            })
            .collect()
    }
}

/// The scalar columns of one row of `v8users`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserRow<'row> {
    /// `Name`.
    pub name: &'row str,
    /// `Descr`.
    pub description: &'row str,
    /// `OSName`, empty when NULL.
    pub os_name: &'row str,
    /// `Email`, empty when NULL or absent from the table.
    pub email: &'row str,
    /// `Show`.
    pub show_in_list: bool,
    /// `EAuth`, false when NULL.
    pub standard_authentication: bool,
    /// `AdmRole`, false when NULL.
    pub administrative: bool,
}

fn malformed(message: &str) -> MetadataError {
    MetadataError::new(
        MetadataErrorKind::Serialization,
        format!("user data: {message}"),
    )
}

/// Decodes the `Data` column of `v8users`.
///
/// # Errors
///
/// Returns [`MetadataError`] when the blob is shorter than its key, is
/// not UTF-8 once decoded, or is not the brace-serialized user record.
pub fn decode_user_data(blob: &[u8]) -> Result<UserData, MetadataError> {
    let (&key_length, rest) = blob.split_first().ok_or_else(|| malformed("empty blob"))?;
    let key_length = usize::from(key_length);
    if key_length == 0 || rest.len() < key_length {
        return Err(malformed("blob shorter than its key"));
    }
    let (key, data) = rest.split_at(key_length);
    let decoded = data
        .iter()
        .zip(key.iter().cycle())
        .map(|(byte, key)| byte ^ key)
        .collect::<Vec<u8>>();
    let record = parse_serialized(&decoded)?;
    let fields = record
        .as_list()
        .ok_or_else(|| malformed("record is not a list"))?;
    let id = fields
        .first()
        .and_then(Value::as_scalar)
        .and_then(|guid| Guid::from_str(guid).ok())
        .ok_or_else(|| malformed("record has no identifier"))?;
    let string_at = |index: usize| {
        fields
            .get(index)
            .and_then(Value::as_string)
            .map(ToOwned::to_owned)
            .unwrap_or_default()
    };
    // The roles block `{N, <guid>…}` is the first nested list.
    let mut roles = Vec::new();
    if let Some(block) = fields.iter().find_map(Value::as_list) {
        let count = block
            .first()
            .and_then(Value::as_u32)
            .ok_or_else(|| malformed("roles block has no count"))?;
        for guid in block.iter().skip(1) {
            let guid = guid
                .as_scalar()
                .and_then(|guid| Guid::from_str(guid).ok())
                .ok_or_else(|| malformed("roles block carries a value that is no identifier"))?;
            roles.push(guid);
        }
        if roles.len() != count as usize {
            return Err(malformed(&format!(
                "roles block declares {count} roles and carries {}",
                roles.len()
            )));
        }
    }
    Ok(UserData {
        id,
        name: string_at(1),
        full_name: string_at(3),
        roles,
        data_separation: decode_data_separation(fields),
        interface_language: decode_interface_language(fields),
        stored_password: decode_stored_password(fields),
    })
}

/// The user's public `StoredPasswordValue`, read from the position
/// measured on 8.3.x `v8users` records: the two digest fields immediately
/// after the roles block plus six scalars (the roles block is the first
/// nested list; `H1` sits seven fields after it, `H2` eight).
///
/// `H1` is the SHA-1 (legacy) or extended digest of the password and `H2`
/// the digest of its ASCII-uppercase form; together they are the
/// `"H1,H2"` value the verifier reads. Extended native records retain these
/// slots and put the algorithm and literal salt in the counted suffix.
/// The earlier packed-string form remains supported. Both digest fields
/// empty means the record has no password slot and yields `None`.
fn decode_stored_password(fields: &[Value]) -> Option<String> {
    let roles_index = fields.iter().position(|value| value.as_list().is_some())?;
    let primary = fields.get(roles_index.checked_add(7)?)?.as_string()?;
    let secondary = fields.get(roles_index.checked_add(8)?)?.as_string()?;
    if primary.is_empty() && secondary.is_empty() {
        return None;
    }
    if let Some(count) = fields.get(17).and_then(Value::as_u32)
        && let Some(optional) = fields
            .get(30usize.checked_add(count as usize)?)
            .and_then(Value::as_u32)
        && optional >= 2
    {
        // Extended shapes must be complete; never reinterpret their main or
        // backup digest slots as legacy credentials if a suffix is malformed.
        return decode_extended_password(fields, roles_index, primary, secondary);
    }
    if fields.len() >= 31 && !has_legacy_password_suffix(fields) {
        // A damaged counter must not hide an extended suffix and make its
        // main digest slots appear to be an unrelated legacy record.
        return None;
    }
    if primary.contains(',') {
        return Some(primary.to_owned());
    }
    Some(format!("{primary},{secondary}"))
}

// Validate the counted tail without coupling authentication to the interface
// language or separator semantics. A corrupt count must not hide an extended
// record, while unrelated language changes must preserve legacy credentials.
fn has_legacy_password_suffix(fields: &[Value]) -> bool {
    let Some(count) = fields.get(17).and_then(Value::as_u32) else {
        return false;
    };
    let Some(optional_index) = 30usize.checked_add(count as usize) else {
        return false;
    };
    let Some(optional @ 0..=1) = fields.get(optional_index).and_then(Value::as_u32) else {
        return false;
    };
    let Some(start) = optional_index.checked_add(1) else {
        return false;
    };
    if optional == 1 && fields.get(start).and_then(Value::as_string).is_none() {
        return false;
    }
    let Some(suffix) = fields.get(start + optional as usize..) else {
        return false;
    };
    match suffix {
        [] => true,
        [first, second, third, fourth, fifth, sixth] => {
            [first, third, fourth, fifth]
                .iter()
                .all(|v| matches!(v, Value::Atom(s) if s == "0"))
                && second.as_string().is_some()
                && sixth.as_string().is_some()
        }
        _ => false,
    }
}

/// Confirmed native shapes: optional blocks of two or five strings followed
/// by [algorithm, literal salt, 0, 0, 0, string]. No algorithm is inferred
/// from digest length, and the legacy backup digests are never consumed.
fn decode_extended_password(
    fields: &[Value],
    roles_index: usize,
    primary: &str,
    secondary: &str,
) -> Option<String> {
    if roles_index != 5 {
        return None;
    }
    for index in [4, 6] {
        Guid::from_str(fields.get(index)?.as_scalar()?).ok()?;
    }
    let Value::Atom(count) = fields.get(17)? else {
        return None;
    };
    let count = count.parse::<usize>().ok()?;
    for binding in fields.get(18..18usize.checked_add(count)?)? {
        let [name, value] = binding.as_list()? else {
            return None;
        };
        name.as_string()?;
        value.as_string()?;
    }
    let optional_index = 30usize.checked_add(count)?;
    let Value::Atom(optional) = fields.get(optional_index)? else {
        return None;
    };
    let optional = optional.parse::<usize>().ok()?;
    if !matches!(optional, 2 | 5) {
        return None;
    }
    let start = optional_index.checked_add(1)?;
    let suffix_start = start.checked_add(optional)?;
    for field in fields.get(start..suffix_start)? {
        field.as_string()?;
    }
    let [Value::Atom(algorithm), salt, third, fourth, fifth, last] = fields.get(suffix_start..)?
    else {
        return None;
    };
    if [third, fourth, fifth]
        .iter()
        .any(|v| !matches!(v, Value::Atom(s) if s == "0"))
    {
        return None;
    }
    last.as_string()?;
    let salt = salt.as_string()?;
    Some(format!("{primary},{secondary},{algorithm},{salt}"))
}

// Позиция блока подтверждена нативным чтением собственных записей форм
// 31/32/37/38. Счётчик необязательного поля и хвост проверяются отдельно:
// простое совпадение длины не доказывает целостность записи.
fn decode_data_separation(fields: &[Value]) -> Option<Vec<(String, String)>> {
    let count = usize::try_from(fields.get(17)?.as_u32()?).ok()?;
    let optional_count =
        usize::try_from(fields.get(30usize.checked_add(count)?)?.as_u32()?).ok()?;
    if optional_count > 1 {
        return None;
    }
    let suffix_start = 31usize.checked_add(count)?.checked_add(optional_count)?;
    if optional_count == 1 {
        fields.get(31usize.checked_add(count)?)?.as_string()?;
    }
    let suffix = fields.get(suffix_start..)?;
    match suffix {
        [] => {}
        [first, second, third, fourth, fifth, sixth]
            if [first, third, fourth, fifth]
                .iter()
                .all(|value| value.as_u32() == Some(0))
                && second.as_string().is_some()
                && sixth.as_string().is_some() => {}
        _ => return None,
    }
    if fields.get(5)?.as_list().is_none()
        || [4, 6].iter().any(|index| {
            fields[*index]
                .as_scalar()
                .and_then(|value| Guid::from_str(value).ok())
                .is_none()
        })
    {
        return None;
    }
    let mut bindings = Vec::new();
    for field in fields.get(18..18usize.checked_add(count)?)? {
        let pair = field.as_list()?;
        let [name, value] = pair else { return None };
        let name = name.as_string()?;
        let value = value.as_string()?;
        if name.is_empty() || bindings.iter().any(|(existing, _)| existing == name) {
            return None;
        }
        bindings.push((name.to_owned(), value.to_owned()));
    }
    Some(bindings)
}

#[cfg(test)]
#[path = "users_separation_tests.rs"]
mod separation_tests;

// Позиция языка измерена для собственных полных форм 31, 37 и 38
// без разделителей. Проверка структуры ниже отвергает неизвестные хвосты.
fn decode_interface_language(fields: &[Value]) -> Option<Guid> {
    if !matches!(fields.len(), 31 | 37 | 38) || fields.get(17)?.as_u32()? != 0 {
        return None;
    }
    decode_data_separation(fields)?;
    let Value::Atom(language) = fields.get(6)? else {
        return None;
    };
    Guid::from_str(language).ok()
}

#[cfg(test)]
#[path = "users_language_tests.rs"]
mod language_tests;

#[cfg(test)]
#[path = "users_legacy_language_tests.rs"]
mod legacy_language_tests;

#[cfg(test)]
#[path = "users_password_tests.rs"]
mod password_tests;
