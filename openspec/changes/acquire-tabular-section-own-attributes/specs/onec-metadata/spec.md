## ADDED Requirements

### Requirement: Acquire own tabular-section attributes
The metadata API SHALL expose `tabular_section_attributes: Option<Vec<Guid>>` on a tabular-section descriptor. A known list SHALL contain every directly owned attribute identity in configuration order, excluding document attributes and other sections. `Some([])` SHALL mean verified empty membership; unknown, duplicate or incomplete collection shapes SHALL produce `None`, without partial success. Attribute declarations and filling settings SHALL use the existing acquisition path and preserve unknown declarations.

#### Scenario: Independent ordered section fields
- **WHEN** a native document contains two sections with different attributes and one empty section
- **THEN** each section exposes exactly its own ordered identities and the empty section exposes `Some([])`

#### Scenario: Incomplete or unknown collection
- **WHEN** the count, header identities or collection shape cannot be completely verified
- **THEN** the section membership is `None` and no shortened list is returned

#### Scenario: Unknown declaration
- **WHEN** a known member has an unknown type declaration
- **THEN** membership remains known and the declaration remains `None`, rather than a fabricated empty type list
