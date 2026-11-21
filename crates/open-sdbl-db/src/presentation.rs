//! The default presentation of a reference: what the platform shows for
//! a catalog element, a document or any other object when the
//! configuration does not say otherwise.
//!
//! The plans are pure: built from the metadata snapshot alone, they are
//! what a console or a protocol server hands the compiler when a query
//! asks for `ПРЕДСТАВЛЕНИЕ` of a reference.

use open_sdbl::metadata::{FieldId, MetadataKind, MetadataSnapshot, ObjectId};
use open_sdbl::query::{PresentationExpression, PresentationPlan};

/// The default presentation plan of `object`: `Наименование (Код)` for a
/// catalog, `<Тип> <Номер> от <Дата>` for a document, a localized value
/// synonym or name for an enumeration, otherwise the first of description,
/// code, number and reference the object has.
#[must_use]
pub fn default_presentation_plan(
    snapshot: &MetadataSnapshot,
    object: ObjectId,
) -> PresentationPlan {
    let metadata_object = snapshot.object_by_id(object);
    let kind = metadata_object.and_then(|object| object.kind);
    let type_name = metadata_object
        .and_then(|metadata_object| metadata_object.presentation("ru"))
        .map_or_else(|| "Документ".to_owned(), str::to_owned);
    let description = snapshot.field_id(object, "Наименование").ok();
    let code = snapshot.field_id(object, "Код").ok();
    let number = snapshot.field_id(object, "Номер").ok();
    let date = snapshot.field_id(object, "Дата").ok();
    let id = snapshot.field_id(object, "Ссылка").ok();

    if kind == Some(MetadataKind::Enumeration)
        && let (Some(metadata_object), Some(id)) = (metadata_object, id)
    {
        let values = snapshot
            .descriptors()
            .iter()
            .filter(|descriptor| {
                descriptor.resource_guid == metadata_object.guid && descriptor.enumeration_value
            })
            .map(|descriptor| {
                let presentation = descriptor
                    .synonyms
                    .iter()
                    .find(|synonym| synonym.language.eq_ignore_ascii_case("ru"))
                    .map(|synonym| synonym.text.trim())
                    .filter(|synonym| !synonym.is_empty())
                    .unwrap_or(&descriptor.name);
                (
                    descriptor.object_guid.to_1c_bytes(),
                    presentation.to_owned(),
                )
            })
            .collect();
        return PresentationPlan {
            object,
            fields: vec![id],
            expression: PresentationExpression::Enumeration(values),
        };
    }

    let (fields, expression) =
        default_presentation_template(kind, &type_name, description, code, number, date, id);
    PresentationPlan {
        object,
        fields,
        expression,
    }
}

/// The fields and expression of a default presentation, from the kind
/// and type name of the object and the standard fields it has.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn default_presentation_template(
    kind: Option<MetadataKind>,
    type_name: &str,
    description: Option<FieldId>,
    code: Option<FieldId>,
    number: Option<FieldId>,
    date: Option<FieldId>,
    id: Option<FieldId>,
) -> (Vec<FieldId>, PresentationExpression) {
    if kind == Some(MetadataKind::Catalog)
        && let (Some(description), Some(code)) = (description, code)
    {
        return (
            vec![description, code],
            PresentationExpression::Concat(vec![
                PresentationExpression::Field(description),
                PresentationExpression::Literal(" (".to_owned()),
                PresentationExpression::Field(code),
                PresentationExpression::Literal(")".to_owned()),
            ]),
        );
    }
    if kind == Some(MetadataKind::Document) {
        return match (number, date) {
            (Some(number), Some(date)) => (
                vec![number, date],
                PresentationExpression::Concat(vec![
                    PresentationExpression::Literal(type_name.to_owned()),
                    PresentationExpression::Literal(" ".to_owned()),
                    PresentationExpression::Field(number),
                    PresentationExpression::Literal(" от ".to_owned()),
                    PresentationExpression::Field(date),
                ]),
            ),
            (Some(number), None) => (
                vec![number],
                PresentationExpression::Concat(vec![
                    PresentationExpression::Literal(type_name.to_owned()),
                    PresentationExpression::Literal(" ".to_owned()),
                    PresentationExpression::Field(number),
                ]),
            ),
            (None, Some(date)) => (
                vec![date],
                PresentationExpression::Concat(vec![
                    PresentationExpression::Literal(type_name.to_owned()),
                    PresentationExpression::Literal(" от ".to_owned()),
                    PresentationExpression::Field(date),
                ]),
            ),
            (None, None) => (
                Vec::new(),
                PresentationExpression::Literal(type_name.to_owned()),
            ),
        };
    }

    match (description, code) {
        (Some(description), Some(code)) => (
            vec![description, code],
            PresentationExpression::Concat(vec![
                PresentationExpression::Field(description),
                PresentationExpression::Literal(" (".to_owned()),
                PresentationExpression::Field(code),
                PresentationExpression::Literal(")".to_owned()),
            ]),
        ),
        (Some(field), None) | (None, Some(field)) => {
            (vec![field], PresentationExpression::Field(field))
        }
        (None, None) => match number.or(id) {
            Some(field) => (vec![field], PresentationExpression::Field(field)),
            None => (Vec::new(), PresentationExpression::Literal(String::new())),
        },
    }
}
