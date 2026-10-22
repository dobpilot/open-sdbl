//! Структурная проверка измеренных прав метаданных без привилегированного режима и SQL.

use std::{error::Error, fmt};

use super::{ConfigDescriptor, Guid, MetadataKind, MetadataSnapshot, Right, RoleRights};

/// Применимость известного права к проверенному объекту метаданных.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataRightApplicability {
    /// Право применимо; разрешение определяется ролями вызывающего пользователя.
    Applicable,
    /// Пара известного права и существующего реквизита не применима.
    NotApplicable,
}

/// Ошибка структурной проверки или неподдержанное правило метаданных.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MetadataRightError {
    /// Имя права неизвестно.
    UnknownRight(String),
    /// В снимке отсутствует запрошенный владелец.
    UnknownOwner(Guid),
    /// Стандартный путь пуст или неизвестен.
    InvalidStandardAttribute(String),
    /// Вид объекта, право или приобретённое описание не покрыты измерениями.
    UnsupportedTarget(String),
    /// Вычисление разрешения запрошено для неприменимого права.
    NotApplicable,
}

impl fmt::Display for MetadataRightError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownRight(name) => write!(f, "unknown metadata right {name:?}"),
            Self::UnknownOwner(owner) => write!(f, "unknown metadata owner {owner}"),
            Self::InvalidStandardAttribute(name) => {
                write!(f, "invalid standard attribute {name:?}")
            }
            Self::UnsupportedTarget(reason) => {
                write!(f, "unsupported metadata right target: {reason}")
            }
            Self::NotApplicable => f.write_str("metadata right is not applicable to this target"),
        }
    }
}

impl Error for MetadataRightError {}

/// Проверенная пара владельца и права с точным измеренным путём реквизита.
///
/// Значение не содержит режима привилегий и не разрешает чтение строк SQL.
/// Его следует использовать с правами того же приобретённого снимка.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedMetadataRight {
    owner: Guid,
    right: Right,
    child: Option<ChildTarget>,
    document: bool,
    applicability: MetadataRightApplicability,
}

// Стандартный реквизит и собственный реквизит документа имеют разные ключи
// в нативном ресурсе роли; UUID собственного реквизита — отдельный объект.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ChildTarget {
    Standard(i64, Guid),
    Attribute(Guid),
}

impl ResolvedMetadataRight {
    /// Возвращает структурную применимость, независимо от ролей и привилегий.
    #[must_use]
    pub const fn applicability(&self) -> MetadataRightApplicability {
        self.applicability
    }

    /// Возвращает известное каноническое право.
    #[must_use]
    pub const fn right(&self) -> &Right {
        &self.right
    }

    /// Проверяет разрешение одной полностью приобретённой роли.
    ///
    /// Родительское разрешение проверяется внутри этой роли до default-прав
    /// реквизита. Объединять результаты разных ролей следует после проверки.
    /// Метод не разворачивает RLS и не проверяет программное чтение полей SQL.
    ///
    /// # Errors
    ///
    /// Возвращает [`MetadataRightError::NotApplicable`] для неприменимого права.
    pub fn granted_by(&self, role: &RoleRights) -> Result<bool, MetadataRightError> {
        self.evaluate(role, false)
    }

    /// Проверяет роль для `VerifyAccessRights` с измеренными независимыми правами.
    ///
    /// При независимых правах явное разрешение дочернего реквизита действует
    /// без разрешения владельца. Для документа измерено также разрешение по
    /// умолчанию при отсутствующей записи. Для справочника такое сочетание
    /// остаётся неподдержанным. Результаты ролей объединяются целиком.
    ///
    /// # Errors
    ///
    /// Возвращает ошибку для неприменимого права или неизмеренного сочетания
    /// независимых и default-прав без разрешения владельца.
    pub fn granted_for_verification(&self, role: &RoleRights) -> Result<bool, MetadataRightError> {
        self.evaluate(role, true)
    }

    fn evaluate(&self, role: &RoleRights, verification: bool) -> Result<bool, MetadataRightError> {
        if self.applicability == MetadataRightApplicability::NotApplicable {
            return Err(MetadataRightError::NotApplicable);
        }
        let parent = role.grants(&self.owner, &self.right);
        let Some(child) = &self.child else {
            return Ok(parent);
        };
        // Родитель проверяется в той же роли до объединения. Для стандартного
        // пути значима вся пара, для собственного реквизита — его UUID без пути.
        let child_entry = role.objects.iter().find(|entry| match child {
            ChildTarget::Standard(kind, id) => {
                entry.object == self.owner
                    && entry.members.len() == 1
                    && entry.members[0].kind == *kind
                    && entry.members[0].id == *id
            }
            ChildTarget::Attribute(id) => entry.object == *id && entry.members.is_empty(),
        });
        if !parent {
            if !verification || !role.independent_rights_of_child_objects {
                return Ok(false);
            }
            if role.set_for_attributes_by_default && (!self.document || child_entry.is_some()) {
                return Err(MetadataRightError::UnsupportedTarget(
                    "unmeasured independent child defaults without parent grant".to_owned(),
                ));
            }
        }
        Ok(child_entry
            .and_then(|entry| entry.right(&self.right))
            .map_or(role.set_for_attributes_by_default, |entry| entry.allowed))
    }
}

/// Проверяет известное право и приобретённый путь до обхода разрешений ролей.
///
/// Измеренная граница: справочник, права `Read`/`View` владельца и стандартных
/// `Код`/`Code`, `Наименование`/`Description` с положительной настроенной длиной.
/// Сравнение имён не зависит от регистра. `Read` существующего стандартного
/// реквизита неприменимо; оператор BSL самостоятельно выбирает обработку этого
/// результата. Для приобретённого документа поддержаны владелец и пути
/// `Номер`/`Number`, `Дата`/`Date`; номер требует положительной настроенной
/// длины. Остальные виды и права не угадываются. Нулевая или отсутствующая
/// длина остаётся явно неподдержанным случаем до нативного контроля.
///
/// # Errors
///
/// Возвращает [`MetadataRightError`] для неизвестного права, владельца, пути
/// или неподдержанной формы метаданных. Ошибка не означает отказ роли.
pub fn resolve_metadata_right(
    snapshot: &MetadataSnapshot,
    owner: &Guid,
    right_name: &str,
    standard_attribute: Option<&str>,
) -> Result<ResolvedMetadataRight, MetadataRightError> {
    let right = Right::parse(right_name)
        .ok_or_else(|| MetadataRightError::UnknownRight(right_name.to_owned()))?;
    let object = snapshot
        .objects()
        .iter()
        .find(|object| &object.guid == owner)
        .ok_or_else(|| MetadataRightError::UnknownOwner(owner.clone()))?;
    if object.kind == Some(MetadataKind::Document) {
        let descriptor = document_descriptor(snapshot, owner)?;
        supported_document_right(owner, &right)?;
        let child = standard_attribute
            .map(|name| {
                let kind = match name.to_lowercase().as_str() {
                    "номер" | "number" => {
                        if !descriptor
                            .document_number
                            .is_some_and(|settings| settings.length > 0)
                        {
                            return Err(MetadataRightError::UnsupportedTarget(format!(
                                "missing or zero-length document number: {owner}"
                            )));
                        }
                        -2
                    }
                    "дата" | "date" => -3,
                    _ => {
                        return Err(MetadataRightError::InvalidStandardAttribute(
                            name.to_owned(),
                        ));
                    }
                };
                Ok(ChildTarget::Standard(kind, standard_member_id()))
            })
            .transpose()?;
        return Ok(document_target(owner, right, child));
    }
    if object.kind != Some(MetadataKind::Catalog) || !matches!(right, Right::Read | Right::View) {
        return Err(MetadataRightError::UnsupportedTarget(format!(
            "{owner}, {right_name}"
        )));
    }
    let member = if let Some(name) = standard_attribute {
        let (kind, code) = match name.to_lowercase().as_str() {
            "код" | "code" => (-2, true),
            "наименование" | "description" => (-3, false),
            _ => {
                return Err(MetadataRightError::InvalidStandardAttribute(
                    name.to_owned(),
                ));
            }
        };
        let settings = snapshot
            .descriptors()
            .iter()
            .find(|descriptor| &descriptor.object_guid == owner)
            .and_then(|descriptor| descriptor.catalog_standard_attributes)
            .ok_or_else(|| {
                MetadataRightError::UnsupportedTarget(format!(
                    "missing catalog standard settings: {owner}"
                ))
            })?;
        let length = if code {
            settings.code_length
        } else {
            settings.description_length
        };
        if length == 0 {
            return Err(MetadataRightError::UnsupportedTarget(format!(
                "unmeasured zero-length standard attribute: {name}"
            )));
        }
        Some(ChildTarget::Standard(kind, standard_member_id()))
    } else {
        None
    };
    let applicability = if member.is_some() && right == Right::Read {
        MetadataRightApplicability::NotApplicable
    } else {
        MetadataRightApplicability::Applicable
    };
    Ok(ResolvedMetadataRight {
        owner: owner.clone(),
        right,
        child: member,
        document: false,
        applicability,
    })
}

/// Разрешает право собственного реквизита приобретённого документа.
///
/// Принадлежность подтверждается полным списком `document_attributes` владельца
/// и единственным приобретённым описателем реквизита. UUID объекта прав берётся
/// из этого описателя, а не из имени, SQL-поля или пути стандартного реквизита.
/// `Read` существующего реквизита неприменимо, `View` применимо. Метод не
/// проверяет роли, привилегии и программное чтение полей SQL.
///
/// # Errors
///
/// Возвращает [`MetadataRightError`] для неизвестного права или владельца,
/// неподдержанного права/вида, неизвестного состава, чужого реквизита,
/// отсутствующего или неоднозначного описания. Ошибка не означает отказ роли.
pub fn resolve_document_attribute_right(
    snapshot: &MetadataSnapshot,
    owner: &Guid,
    attribute: &Guid,
    right_name: &str,
) -> Result<ResolvedMetadataRight, MetadataRightError> {
    let right = Right::parse(right_name)
        .ok_or_else(|| MetadataRightError::UnknownRight(right_name.to_owned()))?;
    let descriptor = document_descriptor(snapshot, owner)?;
    supported_document_right(owner, &right)?;
    if !descriptor
        .document_attributes
        .as_ref()
        .is_some_and(|attributes| attributes.iter().filter(|id| *id == attribute).count() == 1)
    {
        return Err(MetadataRightError::UnsupportedTarget(format!(
            "unknown own document attribute membership: {owner}, {attribute}"
        )));
    }
    acquired_descriptor(snapshot, attribute)?;
    Ok(document_target(
        owner,
        right,
        Some(ChildTarget::Attribute(attribute.clone())),
    ))
}

fn standard_member_id() -> Guid {
    "03f171e8-326f-41c6-9fa5-932a0b12cddf"
        .parse()
        .expect("fixed platform member GUID")
}

fn document_target(
    owner: &Guid,
    right: Right,
    child: Option<ChildTarget>,
) -> ResolvedMetadataRight {
    let applicability = if child.is_some() && right == Right::Read {
        MetadataRightApplicability::NotApplicable
    } else {
        MetadataRightApplicability::Applicable
    };
    ResolvedMetadataRight {
        owner: owner.clone(),
        right,
        child,
        document: true,
        applicability,
    }
}

fn supported_document_right(owner: &Guid, right: &Right) -> Result<(), MetadataRightError> {
    if matches!(right, Right::Read | Right::View) {
        Ok(())
    } else {
        Err(MetadataRightError::UnsupportedTarget(format!(
            "{owner}, {}",
            right.name()
        )))
    }
}

fn document_descriptor<'a>(
    snapshot: &'a MetadataSnapshot,
    owner: &Guid,
) -> Result<&'a ConfigDescriptor, MetadataRightError> {
    let mut objects = snapshot
        .objects()
        .iter()
        .filter(|object| &object.guid == owner);
    let object = objects
        .next()
        .ok_or_else(|| MetadataRightError::UnknownOwner(owner.clone()))?;
    if objects.next().is_some() || object.kind != Some(MetadataKind::Document) {
        return Err(MetadataRightError::UnsupportedTarget(format!(
            "ambiguous or non-document owner: {owner}"
        )));
    }
    acquired_descriptor(snapshot, owner)
}

fn acquired_descriptor<'a>(
    snapshot: &'a MetadataSnapshot,
    id: &Guid,
) -> Result<&'a ConfigDescriptor, MetadataRightError> {
    let mut descriptors = snapshot
        .descriptors()
        .iter()
        .filter(|descriptor| &descriptor.object_guid == id);
    let descriptor = descriptors.next().ok_or_else(|| {
        MetadataRightError::UnsupportedTarget(format!("missing acquired descriptor: {id}"))
    })?;
    if descriptors.next().is_some() {
        return Err(MetadataRightError::UnsupportedTarget(format!(
            "ambiguous acquired descriptor: {id}"
        )));
    }
    Ok(descriptor)
}
