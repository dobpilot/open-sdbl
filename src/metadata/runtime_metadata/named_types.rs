//! Named-type expansion over acquired descriptors.

use super::{ConfigDeclaredType, ConfigDescriptor, Guid};

/// Разворачивает приобретённые определяемые типы один раз до построения снимка.
pub(in crate::metadata) fn resolve_named_types(descriptors: &mut [ConfigDescriptor]) {
    use std::collections::HashMap;
    let mut definitions = HashMap::<Guid, Option<Vec<ConfigDeclaredType>>>::new();
    for d in descriptors.iter() {
        if let Some(id) = &d.defined_type_id {
            definitions
                .entry(id.clone())
                .and_modify(|v| *v = None)
                .or_insert_with(|| d.declared_types.clone());
        }
    }
    if definitions.is_empty() {
        return;
    }
    fn expand(
        types: &[ConfigDeclaredType],
        definitions: &HashMap<Guid, Option<Vec<ConfigDeclaredType>>>,
        path: &mut Vec<Guid>,
    ) -> Option<Vec<ConfigDeclaredType>> {
        let mut result = Vec::new();
        for ty in types {
            if let Some(id) = &ty.reference
                && let Some(definition) = definitions.get(id)
            {
                // Предел глубины совпадает с сериализацией метаданных; цикл и
                // чрезмерное разворачивание не выдаются частичным объявлением.
                if path.len() >= 512 || path.contains(id) {
                    return None;
                }
                path.push(id.clone());
                let members = expand(definition.as_ref()?, definitions, path)?;
                path.pop();
                if result.len().saturating_add(members.len()) > 1_000_000 {
                    return None;
                }
                result.extend(members);
            } else {
                if result.len() >= 1_000_000 {
                    return None;
                }
                result.push(ty.clone());
            }
        }
        Some(result)
    }
    let resolved = definitions
        .iter()
        .map(|(id, types)| {
            (
                id.clone(),
                types
                    .as_ref()
                    .and_then(|types| expand(types, &definitions, &mut vec![id.clone()])),
            )
        })
        .collect::<HashMap<_, _>>();
    fn resolved_types(
        types: &[ConfigDeclaredType],
        resolved: &HashMap<Guid, Option<Vec<ConfigDeclaredType>>>,
    ) -> Option<Vec<ConfigDeclaredType>> {
        let mut output = Vec::new();
        for ty in types {
            if let Some(id) = &ty.reference
                && let Some(definition) = resolved.get(id)
            {
                output.extend(definition.as_ref()?.iter().cloned());
            } else {
                output.push(ty.clone());
            }
            if output.len() > 1_000_000 {
                return None;
            }
        }
        Some(output)
    }
    for d in descriptors.iter_mut() {
        if let Some(types) = &d.declared_types {
            d.declared_types = resolved_types(types, &resolved);
            d.reference_types = d
                .declared_types
                .as_ref()
                .map(|types| types.iter().filter_map(|ty| ty.reference.clone()).collect())
                .unwrap_or_default();
        }
        if let Some(settings) = &mut d.characteristic_settings {
            if let Some(types) = resolved_types(&settings.value_types, &resolved) {
                settings.value_types = types;
            } else {
                d.characteristic_settings = None;
            }
        }
    }
    // Неудачное разворачивание типа реквизита не оставляет обещание
    // полного типизированного состава у владельца.
    let unknown = descriptors
        .iter()
        .filter(|d| d.declared_types.is_none())
        .map(|d| d.object_guid.clone())
        .collect::<std::collections::HashSet<_>>();
    let process_sections = descriptors
        .iter()
        .flat_map(|d| {
            [
                d.business_process_tabular_sections.as_ref(),
                d.data_processor_tabular_sections.as_ref(),
                d.report_tabular_sections.as_ref(),
            ]
        })
        .flatten()
        .flatten()
        .cloned()
        .collect::<std::collections::HashSet<_>>();
    for d in descriptors {
        if process_sections.contains(&d.object_guid)
            && d.tabular_section_attributes
                .as_ref()
                .is_some_and(|ids| ids.iter().any(|id| unknown.contains(id)))
        {
            d.tabular_section_attributes = None;
        }
        for ids in [
            &mut d.document_attributes,
            &mut d.business_process_attributes,
            &mut d.data_processor_attributes,
            &mut d.report_attributes,
            &mut d.exchange_plan_attributes,
            &mut d.accumulation_register_dimensions,
            &mut d.accumulation_register_resources,
            &mut d.information_register_dimensions,
            &mut d.information_register_resources,
        ] {
            if ids
                .as_ref()
                .is_some_and(|ids| ids.iter().any(|id| unknown.contains(id)))
            {
                *ids = None;
            }
        }
        if d.characteristic_attributes
            .as_ref()
            .is_some_and(|ids| ids.iter().any(|id| unknown.contains(id)))
        {
            d.characteristic_attributes = None;
        }
    }
}
