## ADDED Requirements

### Requirement: Acquire document register associations
The metadata API SHALL expose `document_registers: Option<Vec<Guid>>` on the own document descriptor, containing all register object identities from native RegisterRecords in configuration order. It SHALL preserve mixed register classes, SHALL NOT derive membership from SQL, resource names, generated types or incidental GUIDs, and SHALL NOT filter unsupported entries into a partial list. `Some([])` SHALL mean verified empty membership; unknown or incomplete measured shapes SHALL produce `None`.

#### Scenario: Native mixed register collection
- **WHEN** the measured document collection refers to multiple register classes
- **THEN** all own register identities are exposed in native configuration order

#### Scenario: Empty collection
- **WHEN** the measured collection has an exact zero count
- **THEN** the document exposes `Some([])`

#### Scenario: Unknown or incomplete collection
- **WHEN** the class or reference collection is unknown, truncated, duplicated or count-inconsistent
- **THEN** the document exposes `None`, without shortened membership

#### Scenario: Unrelated identities
- **WHEN** unrelated GUIDs occur outside the RegisterRecords collection
- **THEN** they do not enter the register membership
