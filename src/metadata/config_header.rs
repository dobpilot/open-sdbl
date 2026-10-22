//! Обнаружение собственного имени без проекции реквизитов и настроек.

use super::*;

/// Знание об одном сгенерированном типе из измеренной формы заголовка.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigTypeIdentity {
    /// Форма неизвестна либо тип не проецируется для этого класса.
    Unknown,
    /// В измеренной позиции записан пустой GUID.
    Absent,
    /// GUID типа; владельцем является объект заголовка.
    Known(Guid),
}
impl ConfigTypeIdentity {
    pub(crate) fn measured(guid: Guid) -> Self {
        if guid.is_nil() {
            Self::Absent
        } else {
            Self::Known(guid)
        }
    }
}

/// Идентичность табличной части без её реквизитов.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigSectionIdentity {
    /// GUID метаданных части.
    pub object_guid: Guid,
    /// Имя из собственного заголовка части.
    pub name: String,
    /// Подтверждённые типы части; `None` — их назначение не установлено.
    pub generated_types: Option<Vec<Guid>>,
}

/// Объявленное значение перечисления с локализованным представлением.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigValueIdentity {
    /// GUID значения.
    pub object_guid: Guid,
    /// Имя значения из собственного заголовка.
    pub name: String,
    /// Измеренные локализованные синонимы.
    pub synonyms: Vec<Synonym>,
}

/// Собственная идентичность ресурса описания конфигурации.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigHeader {
    /// GUID собственного объекта, совпадающий с именем ресурса.
    pub object_guid: Guid,
    /// Имя из измеренного заголовка; класс задаётся корневой коллекцией.
    pub name: String,
    /// GUID признанных описателей, включая владельца, без повторов.
    /// Задаёт соответствие вложенного GUID этому ресурсу, а не его подробности.
    pub member_guids: Vec<Guid>,
    /// Локализованные синонимы владельца из собственного заголовка.
    pub synonyms: Vec<Synonym>,
    /// Значения перечисления; `None` — неизвестная форма или иной класс.
    pub enumeration_values: Option<Vec<ConfigValueIdentity>>,
    /// Измеренный тип ссылки на точку маршрута бизнес-процесса.
    pub route_point_reference_type: ConfigTypeIdentity,
    /// Измеренный ссылочный тип; неизвестная форма не означает отсутствие.
    pub reference_type: ConfigTypeIdentity,
    /// Измеренная идентичность определяемого типа; неизвестная форма не означает отсутствие.
    pub defined_type: ConfigTypeIdentity,
    /// Измеренный объектный тип обработки, отчёта или бизнес-процесса.
    pub object_type: ConfigTypeIdentity,
    /// Измеренные собственные части; `None` — неизвестный состав.
    pub sections: Option<Vec<ConfigSectionIdentity>>,
}

/// Читает идентичности объекта, типов и частей без подробных описателей.
/// Суффиксные ресурсы возвращают `None` без распаковки. Синтаксис всего
/// описания проверяется; реквизиты и их настройки не проектируются.
///
/// # Errors
/// Возвращает ошибку распаковки, лимита, UTF-8 или сериализации.
pub fn parse_config_header_bounded(
    file_name: &str,
    compressed: &[u8],
    decoded_limit: usize,
) -> Result<Option<ConfigHeader>, MetadataError> {
    let Ok(resource) = Guid::from_str(file_name) else {
        return Ok(None);
    };
    let decoded = inflate_raw_deflate_bounded(compressed, decoded_limit)?;
    let decoded = decoded
        .strip_prefix(&[0xef, 0xbb, 0xbf])
        .unwrap_or(&decoded);
    let text = std::str::from_utf8(decoded)
        .map_err(|error| MetadataError::utf8(error.valid_up_to(), "metadata is not valid UTF-8"))?;
    let mut parser = ConfigParser::new(text, &resource);
    let mut found = None;
    let mut members = Vec::new();
    parser.header_value(0, &mut found, &mut members)?;
    parser.skip_whitespace();
    if parser.offset != text.len() {
        return Err(MetadataError::serialization(
            parser.offset,
            "unexpected trailing metadata",
        ));
    }
    if let Some(header) = &mut found {
        let value = super::super::value::parse_identity_serialized(decoded)?;
        super::super::runtime_metadata::project_header_identity(&value, header);
        header.member_guids = members;
    }
    Ok(found)
}

impl<'input, 'resource> ConfigParser<'input, 'resource> {
    fn header_value(
        &mut self,
        depth: usize,
        found: &mut Option<ConfigHeader>,
        members: &mut Vec<Guid>,
    ) -> Result<ConfigCandidate<'input>, MetadataError> {
        ensure_serialized_depth(depth, self.offset)?;
        self.skip_whitespace();
        match self.current() {
            Some(b'{') => self.header_list(depth, found, members),
            Some(b'"') => Ok(ConfigCandidate::Scalar(SimpleValue::String(self.string()?))),
            Some(b',' | b'}') => Err(MetadataError::serialization(
                self.offset,
                "unexpected metadata delimiter",
            )),
            Some(_) => Ok(ConfigCandidate::Scalar(SimpleValue::Atom(self.atom()?))),
            None => Err(MetadataError::serialization(
                self.offset,
                "empty metadata serialization",
            )),
        }
    }

    fn header_list(
        &mut self,
        depth: usize,
        found: &mut Option<ConfigHeader>,
        members: &mut Vec<Guid>,
    ) -> Result<ConfigCandidate<'input>, MetadataError> {
        self.offset += 1;
        let mut simple = Some(Vec::new());
        let mut window = Vec::new();
        let mut count = 0;
        let mut expecting = true;
        loop {
            self.skip_whitespace();
            let candidate = match self.current() {
                None => {
                    return Err(MetadataError::serialization(
                        self.offset,
                        "unterminated metadata list",
                    ));
                }
                Some(b'}') => {
                    if expecting && count != 0 {
                        self.header_candidate(
                            ConfigCandidate::Scalar(SimpleValue::Null),
                            &mut simple,
                            &mut window,
                            found,
                            members,
                        );
                    }
                    self.offset += 1;
                    break;
                }
                Some(b',') if expecting => {
                    self.offset += 1;
                    ConfigCandidate::Scalar(SimpleValue::Null)
                }
                Some(b',') => {
                    self.offset += 1;
                    expecting = true;
                    continue;
                }
                Some(_) if expecting => {
                    let candidate = self.header_value(depth + 1, found, members)?;
                    expecting = false;
                    candidate
                }
                Some(_) => {
                    return Err(MetadataError::serialization(
                        self.offset,
                        "expected ',' or '}' in metadata list",
                    ));
                }
            };
            self.header_candidate(candidate, &mut simple, &mut window, found, members);
            count += 1;
        }
        Ok(simple.map_or(ConfigCandidate::Other, ConfigCandidate::List))
    }

    fn header_candidate(
        &self,
        candidate: ConfigCandidate<'input>,
        simple: &mut Option<Vec<SimpleValue<'input>>>,
        window: &mut Vec<ConfigCandidate<'input>>,
        found: &mut Option<ConfigHeader>,
        members: &mut Vec<Guid>,
    ) {
        if let Some(values) = simple {
            if values.len() < 3
                && let Some(value) = candidate.as_scalar()
            {
                values.push(value.clone());
            } else {
                *simple = None;
            }
        }
        window.push(candidate);
        if window.len() == 3 {
            if let Some((guid, _, _)) = descriptor_identity(&window[0], &window[1])
                && !members.contains(&guid)
            {
                members.push(guid);
            }
            if found.is_none()
                && let Some((guid, name, _)) = descriptor_identity(&window[0], &window[1])
                && &guid == self.resource_guid
            {
                *found = Some(ConfigHeader {
                    object_guid: guid,
                    name: name.to_owned(),
                    reference_type: ConfigTypeIdentity::Unknown,
                    object_type: ConfigTypeIdentity::Unknown,
                    defined_type: ConfigTypeIdentity::Unknown,
                    sections: None,
                    member_guids: Vec::new(),
                    synonyms: Vec::new(),
                    enumeration_values: None,
                    route_point_reference_type: ConfigTypeIdentity::Unknown,
                });
            }
            window.remove(0);
        }
    }
}
