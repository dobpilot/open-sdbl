## ADDED Requirements

### Requirement: Decode common-attribute separation settings
The metadata decoder SHALL project, from a bare-GUID Config resource whose
class id is the common-attribute class, the separated-data-use mode
(`Independent` or `IndependentAndShared`), the GUID of the session
parameter bound as the separator value, and the GUID of the session
parameter bound as the use flag. Resolution SHALL attach the settings to
the separator field, translating the GUIDs into the session parameter
names found in their own descriptors, and SHALL expose every separator
field through `MetadataSnapshot::separators`. A separator whose resource
does not match the layout SHALL resolve as `IndependentAndShared` without
bindings and SHALL be reported as a `SeparatorSettingsMissing` finding.

#### Scenario: Bound BSP separator
- **WHEN** the common attribute `ОбластьДанныхОсновныеДанные` references
  the session parameters `ОбластьДанныхЗначение` and
  `ОбластьДанныхИспользование`
- **THEN** its field carries mode `IndependentAndShared`, value parameter
  `ОбластьДанныхЗначение`, and use parameter `ОбластьДанныхИспользование`

#### Scenario: Truncated resource
- **WHEN** a separator's Config resource ends before the binding
  references
- **THEN** resolution succeeds, the field carries no bindings, and the
  report lists a `SeparatorSettingsMissing` finding naming the attribute
