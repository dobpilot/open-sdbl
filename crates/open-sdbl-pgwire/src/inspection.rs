//! Logical paths for resources in the active base configuration.

mod classes;
pub(crate) mod dcs;

use std::collections::{BTreeMap, HashMap};

use open_sdbl::metadata::{
    Guid, Value, decode_managed_form_module, decode_module_source, inflate_raw_deflate_bounded,
    parse_config_header_bounded, parse_serialized, read_container_entry,
};
use open_sdbl_db::{ConfigResource, DatabaseSession, DbError};

use crate::codec::{BackendWriter, ErrorFields};
use classes::{Child, classes};

pub(crate) const RESOURCE_LIMIT: usize = 16 * 1024 * 1024;
const HEADER_LIMIT: usize = 2 * 1024 * 1024;
const DCS_SCHEMA_NS: &str = "http://v8.1c.ru/8.1/data-composition-system/schema";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Module,
    Form,
    FormModule,
    Xml,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Source {
    file: String,
    format: Format,
}

#[derive(Default)]
pub(crate) struct Index {
    paths: BTreeMap<String, Entry>,
    report_schemas: HashMap<String, Option<Guid>>,
    templates: HashMap<Guid, Option<String>>,
    incomplete: bool,
}

struct Entry {
    path: String,
    source: Option<Source>,
    ambiguous: bool,
}

// Only resource names from Config enter the addressed database API.
struct Names(HashMap<String, String>);

impl Names {
    fn new(names: Vec<String>) -> Self {
        Self(
            names
                .into_iter()
                .map(|name| (name.to_ascii_lowercase(), name))
                .collect(),
        )
    }

    fn resource(&self, id: &Guid, suffix: &str) -> Option<&str> {
        self.0.get(&format!("{id}{suffix}")).map(String::as_str)
    }
}

fn invalid(message: &str) -> DbError {
    DbError::Data(message.to_owned())
}

fn list(value: &Value) -> Option<&[Value]> {
    match value {
        Value::List(values) => Some(values),
        _ => None,
    }
}

fn atom(value: &Value) -> Option<&str> {
    match value {
        Value::Atom(value) => Some(value),
        _ => None,
    }
}

fn guid(value: &Value) -> Option<Guid> {
    atom(value)?.parse::<Guid>().ok().filter(|id| !id.is_nil())
}

fn count(value: &Value) -> Option<usize> {
    atom(value)?.parse().ok()
}

fn descriptor(root: &ConfigResource) -> Result<Guid, DbError> {
    let value = decoded(root)?;
    let fields = list(&value).ok_or_else(|| invalid("invalid configuration root"))?;
    fields
        .get(1)
        .and_then(guid)
        .ok_or_else(|| invalid("invalid configuration root descriptor"))
}

fn decoded(resource: &ConfigResource) -> Result<Value, DbError> {
    let bytes = inflate_raw_deflate_bounded(&resource.compressed, HEADER_LIMIT)?;
    Ok(parse_serialized(&bytes)?)
}

fn collection(fields: &[Value]) -> Option<(&str, Vec<Guid>)> {
    let [class, size, members @ ..] = fields else {
        return None;
    };
    if count(size)? != members.len() {
        return None;
    }
    Some((
        atom(class)?,
        members.iter().map(guid).collect::<Option<_>>()?,
    ))
}

fn root_objects(value: &Value) -> Option<Vec<(&str, Guid)>> {
    let [_, _, size, groups @ ..] = list(value)? else {
        return None;
    };
    let mut result = Vec::new();
    for group in groups.get(..count(size)?)? {
        let body = list(list(group)?.get(1)?)?;
        let body = if body.first().and_then(atom) == Some("6") {
            list(body.get(1)?)?
        } else {
            body
        };
        let [_, _, size, collections @ ..] = body else {
            return None;
        };
        for row in collections.get(..count(size)?)? {
            let (class, members) = collection(list(row)?)?;
            result.extend(members.into_iter().map(|id| (class, id)));
        }
    }
    Some(result)
}

fn configuration_identity(value: &Value) -> Option<(Guid, &str)> {
    let general = list(list(value)?.get(3)?)?;
    if general.first().and_then(atom) != Some("9cd510cd-abfc-11d4-9434-004095e12fc7") {
        return None;
    }
    let mut fields = general;
    for _ in 0..4 {
        fields = list(fields.get(1)?)?;
    }
    let id = guid(list(fields.get(1)?)?.get(2)?)?;
    let Value::String(name) = fields.get(2)? else {
        return None;
    };
    Some((id, name))
}

fn header(resource: &ConfigResource) -> Result<String, DbError> {
    parse_config_header_bounded(&resource.file_name, &resource.compressed, HEADER_LIMIT)?
        .map(|header| header.name)
        .filter(|name| !name.is_empty() && !name.contains(['.', '\0']))
        .ok_or_else(|| invalid("missing configuration object name"))
}

enum Member {
    Resource(Child, Guid),
    Inline(Child, Guid, String),
}

fn inline_identity(value: &Value) -> Option<(Guid, &str)> {
    let fields = list(value)?;
    if let [
        version,
        reference,
        Value::String(name),
        _synonym,
        _comment,
        ..,
    ] = fields
        && matches!(atom(version), Some("1" | "2" | "3"))
        && let Some([marker, zero, id]) = list(reference)
        && matches!(atom(marker), Some("0" | "1"))
        && atom(zero) == Some("0")
        && !name.is_empty()
        && !name.contains(['.', '\0'])
    {
        if let Some(id) = guid(id) {
            return Some((id, name));
        }
    }
    fields.iter().find_map(inline_identity)
}

fn child_objects(value: &Value) -> Result<Vec<Member>, DbError> {
    let Some([_, _, size, rows @ ..]) = list(value) else {
        return Ok(Vec::new());
    };
    let mut children = Vec::new();
    // Some standalone objects have different headers and no child collections.
    // Only recognized direct collections can attach a child to this owner.
    for (i, row) in rows.iter().enumerate() {
        let Some(fields) = list(row) else { continue };
        let Some(child) = fields.first().and_then(atom).and_then(Child::from_class) else {
            continue;
        };
        let [_, length, members @ ..] = fields else {
            return Err(invalid("invalid configuration child collection"));
        };
        if count(length) != Some(members.len()) || !count(size).is_some_and(|n| i < n) {
            return Err(invalid("invalid configuration child collection"));
        }
        for member in members {
            if let Some(id) = guid(member) {
                children.push(Member::Resource(child, id));
            } else if let Some((id, name)) = inline_identity(member) {
                children.push(Member::Inline(child, id, name.to_owned()));
            } else {
                return Err(invalid("invalid configuration member identity"));
            }
        }
    }
    Ok(children)
}

// Designer's report profile 19 stores a template GUID in slot 5. NIL is
// an unset property. An unrecognized descriptor must not imply absence.
fn report_main_schema(value: &Value, owner: &Guid) -> Option<Option<Guid>> {
    let class = list(list(value)?.get(1)?)?;
    if class.len() != 18 || atom(class.first()?) != Some("19") {
        return None;
    }
    let [marker, header] = list(&class[3])? else {
        return None;
    };
    let [version, identity, ..] = list(header)? else {
        return None;
    };
    let [kind, zero, id] = list(identity)? else {
        return None;
    };
    if atom(marker) != Some("0")
        || atom(version) != Some("3")
        || atom(kind) != Some("1")
        || atom(zero) != Some("0")
        || guid(id).as_ref() != Some(owner)
    {
        return None;
    }
    let target = atom(&class[5])?.parse::<Guid>().ok()?;
    Some((!target.is_nil()).then_some(target))
}

async fn read_one(session: &mut DatabaseSession, name: &str) -> Result<ConfigResource, DbError> {
    session
        .configuration_resources(&[name])
        .await?
        .pop()
        .ok_or_else(|| invalid("configuration resource is missing"))
}

fn report_skipped(progress: &mut dyn open_sdbl_db::MetadataProgress, file: &str, error: &DbError) {
    if let DbError::Metadata(error) = error {
        progress.skipped_config(file, error.kind());
    }
}

impl Index {
    pub(crate) async fn acquire(
        session: &mut DatabaseSession,
        progress: &mut dyn open_sdbl_db::MetadataProgress,
    ) -> Result<Self, DbError> {
        let names = Names::new(session.configuration_resource_names().await?);
        let versions = if let Some(file) = names.0.get("versions") {
            Some(read_one(session, file).await?)
        } else {
            None
        };
        let root = read_one(session, "root").await?;
        let id = descriptor(&root).inspect_err(|error| report_skipped(progress, "root", error))?;
        let descriptor = read_one(session, names.resource(&id, "").unwrap_or(id.as_str())).await?;
        let configuration = decoded(&descriptor)
            .inspect_err(|error| report_skipped(progress, &descriptor.file_name, error))?;
        let objects = root_objects(&configuration)
            .ok_or_else(|| invalid("invalid configuration object catalog"))?;

        let mut index = Self::default();
        index.add_configuration(&configuration, &names);
        let mut children = Vec::new();
        for batch in objects.chunks(64) {
            let present = batch
                .iter()
                .filter_map(|(class, id)| names.resource(id, "").map(|file| (class, id, file)))
                .collect::<Vec<_>>();
            let requested = present.iter().map(|(_, _, file)| *file).collect::<Vec<_>>();
            index.incomplete |= present.len() != batch.len();
            let resources = session.configuration_resources(&requested).await?;
            let resources = resources
                .iter()
                .map(|r| (r.file_name.as_str(), r))
                .collect::<HashMap<_, _>>();
            for (class, id, file) in present {
                if let Err(error) =
                    index.add_owner(class, id, resources[file], &names, &mut children)
                {
                    index.incomplete = true;
                    report_skipped(progress, file, &error);
                }
            }
        }
        for batch in children.chunks(64) {
            let present = batch
                .iter()
                .filter_map(|(owner, kind, id)| {
                    names.resource(id, "").map(|file| (owner, kind, id, file))
                })
                .collect::<Vec<_>>();
            let requested = present
                .iter()
                .map(|(_, _, _, file)| *file)
                .collect::<Vec<_>>();
            index.incomplete |= present.len() != batch.len();
            let resources = session.configuration_resources(&requested).await?;
            let resources = resources
                .iter()
                .map(|r| (r.file_name.as_str(), r))
                .collect::<HashMap<_, _>>();
            for (owner, kind, id, file) in present {
                if let Err(error) = index.add_child(owner, *kind, id, resources[file], &names) {
                    index.incomplete = true;
                    report_skipped(progress, file, &error);
                }
            }
        }
        if let Some(versions) = versions
            && read_one(session, &versions.file_name).await? != versions
        {
            return Err(invalid("configuration changed during metadata indexing"));
        }
        Ok(index)
    }

    fn add_configuration(&mut self, configuration: &Value, names: &Names) {
        if let Some((id, name)) = configuration_identity(configuration) {
            let path = format!("Конфигурация.{name}");
            self.insert(path.clone(), None);
            self.add_body(
                &path,
                "МодульУправляемогоПриложения.bsl",
                &id,
                ".6",
                Format::Module,
                names,
            );
            self.add_body(&path, "МодульСеанса.bsl", &id, ".7", Format::Module, names);
        }
    }

    fn add_owner(
        &mut self,
        class: &str,
        id: &Guid,
        resource: &ConfigResource,
        names: &Names,
        children: &mut Vec<(String, Child, Guid)>,
    ) -> Result<(), DbError> {
        let class_id = class;
        let class = classes(class_id);
        let prefix = class.as_ref().map_or_else(
            || format!("Класс_{class_id}"),
            |class| class.name.to_owned(),
        );
        let path = format!("{prefix}.{}", header(resource)?);
        self.insert(path.clone(), None);
        if let Some(class) = class {
            for (suffix, label) in class.modules {
                self.add_body(&path, label, id, suffix, Format::Module, names);
            }
            if matches!(class.name, "Обработка" | "Отчет") {
                self.set_body(&path, id, ".0", Format::Module, names);
            }
            if let Some(kind) = class.body {
                self.add_child_body(&path, kind, id, names);
            }
        }
        let decoded = decoded(resource)?;
        if prefix == "Отчет"
            && let Some(target) = report_main_schema(&decoded, id)
        {
            self.report_schemas.insert(path.to_lowercase(), target);
        }
        for child in child_objects(&decoded)? {
            match child {
                Member::Resource(kind, id) => children.push((path.clone(), kind, id)),
                Member::Inline(kind, id, name) => {
                    self.register_child(&path, kind, &id, &name, names)
                }
            }
        }
        Ok(())
    }

    fn add_child(
        &mut self,
        owner: &str,
        kind: Child,
        id: &Guid,
        resource: &ConfigResource,
        names: &Names,
    ) -> Result<(), DbError> {
        self.register_child(owner, kind, id, &header(resource)?, names);
        Ok(())
    }

    fn register_child(&mut self, owner: &str, kind: Child, id: &Guid, name: &str, names: &Names) {
        let path = format!("{owner}.{}.{name}", kind.group());
        self.insert(path.clone(), None);
        self.add_child_body(&path, kind, id, names);
    }

    fn add_child_body(&mut self, path: &str, kind: Child, id: &Guid, names: &Names) {
        match kind {
            Child::Form => {
                self.set_body(path, id, ".0", Format::Form, names);
                self.add_body(path, "Модуль.bsl", id, ".0", Format::FormModule, names);
            }
            Child::Template => {
                self.set_body(path, id, ".0", Format::Xml, names);
                self.templates
                    .entry(id.clone())
                    .and_modify(|entry| *entry = None)
                    .or_insert_with(|| Some(path.to_owned()));
            }
            Child::Command => self.add_body(path, "Модуль.bsl", id, ".0", Format::Module, names),
            Child::Descriptor(_) => {}
        }
    }

    fn add_body(
        &mut self,
        path: &str,
        label: &str,
        id: &Guid,
        suffix: &str,
        format: Format,
        names: &Names,
    ) {
        if let Some(file) = names.resource(id, suffix) {
            self.insert(
                format!("{path}.{label}"),
                Some(Source {
                    file: file.into(),
                    format,
                }),
            );
        }
    }

    fn set_body(&mut self, path: &str, id: &Guid, suffix: &str, format: Format, names: &Names) {
        if let Some(file) = names.resource(id, suffix) {
            self.paths
                .get_mut(&path.to_lowercase())
                .expect("registered path")
                .source = Some(Source {
                file: file.into(),
                format,
            });
        }
    }

    fn insert(&mut self, path: String, source: Option<Source>) {
        self.paths
            .entry(path.to_lowercase())
            .and_modify(|entry| entry.ambiguous = true)
            .or_insert(Entry {
                path,
                source,
                ambiguous: false,
            });
    }

    pub(crate) fn find(
        &self,
        filter: &str,
        budget: usize,
        limit: usize,
    ) -> Result<Vec<Vec<u8>>, ErrorFields> {
        let filter = filter.to_lowercase();
        let mut remaining = budget;
        let mut rows = Vec::new();
        for (key, entry) in &self.paths {
            if !key.contains(&filter) || is_hidden_attribute_path(key) {
                continue;
            }
            remaining = remaining
                .checked_sub(entry.path.len() + 11 + size_of::<Vec<u8>>())
                .filter(|_| rows.len() < limit)
                .ok_or_else(|| {
                    ErrorFields::error("54000", "metadata result exceeds the gateway limit")
                })?;
            rows.push(text_row(&entry.path));
        }
        Ok(rows)
    }

    pub(crate) fn incomplete(&self) -> bool {
        self.incomplete
    }

    pub(crate) fn source(&self, path: &str) -> Result<Source, ErrorFields> {
        let path = canonical_path(path);
        self.entry(&path)?.source.clone().ok_or_else(unsupported)
    }

    pub(crate) fn metadata_ref(
        &self,
        owner: &str,
        property: &str,
    ) -> Result<Option<String>, ErrorFields> {
        let owner = canonical_path(owner);
        self.entry(&owner)?;
        if !matches!(
            property.to_lowercase().as_str(),
            "основнаясхемакомпоновкиданных" | "maindatacompositionschema"
        ) {
            return Err(unsupported_property());
        }
        let Some(id) = self
            .report_schemas
            .get(&owner)
            .ok_or_else(unsupported_property)?
        else {
            return Ok(None);
        };
        let target = match self.templates.get(id) {
            Some(Some(path)) => path,
            Some(None) => {
                return Err(ErrorFields::error(
                    "42725",
                    "template identity is ambiguous",
                ));
            }
            None => {
                return Err(ErrorFields::error(
                    "42704",
                    "configured schema template was not found",
                ));
            }
        };
        self.entry(&target.to_lowercase())?;
        Ok(Some(target.clone()))
    }

    fn entry(&self, key: &str) -> Result<&Entry, ErrorFields> {
        let entry = self
            .paths
            .get(key)
            .ok_or_else(|| ErrorFields::error("42704", "metadata path was not found"))?;
        if entry.ambiguous {
            return Err(ErrorFields::error("42725", "metadata path is ambiguous"));
        }
        Ok(entry)
    }
}

fn canonical_path(path: &str) -> String {
    let path = path.to_lowercase();
    match path.split_once('.') {
        Some(("отчеты" | "отчёты" | "отчёт" | "report", rest)) => {
            format!("отчет.{rest}")
        }
        _ => path,
    }
}

fn unsupported_property() -> ErrorFields {
    ErrorFields::error("0A000", "the metadata reference property is unsupported")
}

fn is_hidden_attribute_path(key: &str) -> bool {
    let mut segments = key.split('.');
    let object_kind = segments.next();
    !matches!(object_kind, Some("обработка" | "отчет")) && segments.nth(1) == Some("реквизиты")
}

fn unsupported() -> ErrorFields {
    ErrorFields::error(
        "0A000",
        "textual BSL or XML is unavailable for this metadata path",
    )
}

pub(crate) fn text_row(value: &str) -> Vec<u8> {
    let mut row = BackendWriter::new();
    row.data_row([Some(value.as_bytes())]);
    row.take()
}

impl Source {
    pub(crate) fn file(&self) -> &str {
        &self.file
    }

    pub(crate) fn decode(
        &self,
        resource: &ConfigResource,
        limit: usize,
    ) -> Result<String, ErrorFields> {
        let bytes =
            inflate_raw_deflate_bounded(&resource.compressed, limit).map_err(|_| unsupported())?;
        match self.format {
            Format::Module => decode_module_source(&bytes, limit).map_err(|_| unsupported()),
            Format::Form => xml_source(&bytes, limit)
                .or_else(|| form_module(&bytes, limit))
                .ok_or_else(unsupported),
            Format::FormModule => form_module(&bytes, limit).ok_or_else(unsupported),
            Format::Xml => xml_source(&bytes, limit).ok_or_else(unsupported),
        }
    }
}

fn form_module(bytes: &[u8], limit: usize) -> Option<String> {
    decode_managed_form_module(bytes, limit)
        .or_else(|_| decode_module_source(bytes, limit))
        .ok()
}

fn xml_source(bytes: &[u8], limit: usize) -> Option<String> {
    if let Some(xml) = checked_xml(bytes) {
        return Some(xml.to_owned());
    }
    if bytes.starts_with(&[255, 255, 255, 127]) {
        let xml = read_container_entry(bytes, "text", limit).ok()??;
        return checked_xml(&xml).map(str::to_owned);
    }
    composition_schema(bytes).map(str::to_owned)
}

fn checked_xml(bytes: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes)).ok()?;
    parse_xml(text).ok()?;
    Some(text)
}

fn parse_xml(text: &str) -> Result<roxmltree::Document<'_>, roxmltree::Error> {
    let options = roxmltree::ParsingOptions {
        nodes_limit: 100_000,
        ..Default::default()
    };
    roxmltree::Document::parse_with_options(text, options)
}

// Config stores the raw schema XML first, followed by a length-delimited XML
// stream for each settings variant and an auxiliary XML document.
fn composition_schema(bytes: &[u8]) -> Option<&str> {
    let header = bytes.get(..8)?;
    if header[..4] != [0; 4] {
        return None;
    }
    let variants = u32::from_le_bytes(header[4..].try_into().ok()?) as usize;
    let lengths_end = variants.checked_add(2)?.checked_mul(8)?;
    let lengths = bytes.get(8..lengths_end)?;
    let mut payload = bytes.get(lengths_end..)?;
    let mut schema = None;
    for (i, length) in lengths.chunks_exact(8).enumerate() {
        let len = usize::try_from(u64::from_le_bytes(length.try_into().ok()?)).ok()?;
        let (document, rest) = payload.split_at_checked(len)?;
        let xml = checked_xml(document)?;
        if i == 0 {
            let doc = roxmltree::Document::parse(xml).ok()?;
            if doc.root_element().tag_name().name() != "SchemaFile"
                || !doc
                    .root_element()
                    .children()
                    .any(|node| node.has_tag_name((DCS_SCHEMA_NS, "dataCompositionSchema")))
            {
                return None;
            }
            schema = Some(xml);
        }
        payload = rest;
    }
    checked_xml(payload)?;
    schema
}

#[cfg(test)]
#[test]
fn metadata_search_lists_attribute_paths_only_for_reports_and_data_processors() {
    let mut index = Index::default();
    let processing = "Обработка.Загрузка.Реквизиты.Файл";
    let report = "Отчет.Остатки.Реквизиты.Период";
    let form = "Справочник.Номенклатура.Формы.Реквизиты.Модуль.bsl";
    for path in [
        processing,
        report,
        form,
        "Справочник.Номенклатура.Реквизиты.Артикул",
        "Документ.Заказ.Реквизиты.Комментарий",
        "РегистрСведений.Ячейки.Реквизиты.Описание",
    ] {
        index.insert(path.into(), None);
    }

    for filter in ["", "РЕКВИЗИТЫ"] {
        assert_eq!(
            index.find(filter, 4096, 3).unwrap(),
            [processing, report, form].map(text_row)
        );
    }
}

#[cfg(test)]
pub(crate) mod fixture {
    use super::*;

    const CF: &[u8] = include_bytes!("../../../tests/fixtures/data-processor-header2/control.cf");
    const DCS: &str = "59aadc03-adbc-481f-89bc-d7f391789522";
    const REPORT: &str = "8bac013d-e4dc-43c2-980d-6190972b6678";
    const SELECTED_TEMPLATE: &str = "326a84f6-0fa8-4460-9834-0da4dc917364";
    const DEFAULT_NAME_TEMPLATE: &str = "9e805e6f-5d46-4b98-bd47-3a5e2c9a9cc4";
    pub(crate) const REPORT_PATH: &str = "Отчет.ЗамерОтчета";
    pub(crate) const REPORT_SCHEMA: &str = "Отчет.ЗамерОтчета.Макеты.ВыбраннаяСхема";
    pub(crate) const OWNER: &str = "Обработка.ПомощникСозданияАвтономногоРабочегоМеста";
    pub(crate) const FORM: &str = "Обработка.ПомощникСозданияАвтономногоРабочегоМеста.Формы.ДополнительноеОписание.Модуль.bsl";
    pub(crate) const TEMPLATE: &str =
        "Обработка.ПомощникСозданияАвтономногоРабочегоМеста.Макеты.ИнструкцияПоНастройкеАРМ";

    pub(crate) fn read(name: &str) -> Result<ConfigResource, DbError> {
        let report_source = match name {
            REPORT => Some(
                include_bytes!("../../../tests/fixtures/pgwire-report-schema/report.deflate")
                    .as_slice(),
            ),
            SELECTED_TEMPLATE => Some(
                include_bytes!(
                    "../../../tests/fixtures/pgwire-report-schema/selected-template.deflate"
                )
                .as_slice(),
            ),
            DEFAULT_NAME_TEMPLATE => Some(
                include_bytes!(
                    "../../../tests/fixtures/pgwire-report-schema/default-name-template.deflate"
                )
                .as_slice(),
            ),
            n if n.strip_suffix(".0") == Some(SELECTED_TEMPLATE) => Some(
                include_bytes!(
                    "../../../tests/fixtures/pgwire-report-schema/selected-schema.deflate"
                )
                .as_slice(),
            ),
            _ => None,
        };
        let compressed = if let Some(source) = report_source {
            source.into()
        } else if name == DCS {
            include_bytes!("../../../tests/fixtures/pgwire-metadata-inspection/dcs.config.deflate")
                .as_slice()
                .into()
        } else if name.strip_suffix(".0") == Some(DCS) {
            include_bytes!("../../../tests/fixtures/pgwire-metadata-inspection/dcs.deflate")
                .as_slice()
                .into()
        } else {
            read_container_entry(CF, name, RESOURCE_LIMIT)?
                .ok_or_else(|| invalid("fixture resource missing"))?
                .into()
        };
        Ok(ConfigResource {
            file_name: name.into(),
            compressed,
        })
    }

    pub(crate) fn report_index() -> Index {
        let mut index = Index::default();
        let mut children = Vec::new();
        let owner = read(REPORT).unwrap();
        let id = REPORT.parse().unwrap();
        let names = Names::new(vec![format!("{SELECTED_TEMPLATE}.0")]);
        index
            .add_owner(
                "631b75a0-29e2-11d6-a3c7-0050bae0a776",
                &id,
                &owner,
                &names,
                &mut children,
            )
            .unwrap();
        for (path, kind, id) in children {
            index
                .add_child(&path, kind, &id, &read(id.as_str()).unwrap(), &names)
                .unwrap();
        }
        index
    }

    #[test]
    fn main_schema_uses_the_native_report_property() {
        let designer =
            include_str!("../../../tests/fixtures/pgwire-report-schema/report.designer.xml");
        let designer = roxmltree::Document::parse(designer).unwrap();
        let target = designer
            .descendants()
            .find(|node| node.has_tag_name("MainDataCompositionSchema"))
            .and_then(|node| node.text())
            .unwrap();
        assert_eq!(target, "Report.ЗамерОтчета.Template.ВыбраннаяСхема");
        let index = report_index();
        assert_eq!(
            index
                .metadata_ref(REPORT_PATH, "ОсновнаяСхемаКомпоновкиДанных")
                .unwrap()
                .as_deref(),
            Some(REPORT_SCHEMA)
        );
        assert!(
            index
                .entry("отчет.замеротчета.макеты.основнаясхемакомпоновкиданных")
                .is_ok()
        );

        let empty = ConfigResource {
            file_name: REPORT.into(),
            compressed: include_bytes!("../../../tests/fixtures/report-object/empty.deflate")
                .as_slice()
                .into(),
        };
        assert_eq!(
            report_main_schema(&decoded(&empty).unwrap(), &REPORT.parse().unwrap()),
            Some(None)
        );
    }

    pub(crate) fn index() -> Index {
        let root = read("root").unwrap();
        let descriptor = read(descriptor(&root).unwrap().as_str()).unwrap();
        let value = decoded(&descriptor).unwrap();
        let objects = root_objects(&value).expect("native catalog");
        let mut children = Vec::new();
        let mut index = Index::default();
        for (class, owner) in objects {
            let mut suffixes = classes(class).map_or_else(Vec::new, |class| {
                class.modules.iter().map(|(s, _)| *s).collect::<Vec<_>>()
            });
            suffixes.push(".0");
            let names = names(&owner, &suffixes);
            index
                .add_owner(
                    class,
                    &owner,
                    &read(owner.as_str()).unwrap(),
                    &names,
                    &mut children,
                )
                .unwrap_or_else(|error| panic!("failed owner {class} / {owner}: {error}"));
        }
        for (path, kind, id) in children {
            let resource = read(id.as_str()).unwrap();
            index
                .add_child(&path, kind, &id, &resource, &names(&id, &[".0"]))
                .unwrap();
        }
        index
    }

    fn names(id: &Guid, suffixes: &[&str]) -> Names {
        Names::new(
            suffixes
                .iter()
                .map(|suffix| format!("{id}{suffix}"))
                .filter(|name| read(name).is_ok())
                .collect(),
        )
    }
}
