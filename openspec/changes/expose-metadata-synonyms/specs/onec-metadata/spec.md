## ADDED Requirements

### Requirement: Expose localized synonyms and comments on resolved metadata
Resolution SHALL carry the localized synonyms and the descriptor comment of
a `Config` descriptor onto the metadata object or field it names, in the
source order the descriptor lists them. An object or field with no
descriptor, or one whose descriptor carries neither, SHALL expose an empty
synonym list and no comment rather than failing.

Both SHALL answer a synonym by language code, compared
case-insensitively, with surrounding whitespace trimmed and an empty
result reported as absent. Both SHALL answer a presentation for a
language: the synonym when there is one, otherwise the metadata name. The
snapshot SHALL answer the synonym and the presentation of an object
addressed by its identifier.

#### Scenario: Attribute synonym
- **WHEN** a bare-GUID resource contains `{1,0,<guid>},"КоррСчет",{2,"ru","Корр. счет"}`
- **THEN** the resolved field carries the `ru` synonym `Корр. счет`, and its
  Russian presentation is `Корр. счет` while its name stays `КоррСчет`

#### Scenario: Language without a synonym
- **WHEN** a descriptor carries only a `ru` synonym and the caller asks for
  `en`
- **THEN** the synonym is absent and the presentation falls back to the
  metadata name

#### Scenario: Blank synonym
- **WHEN** a descriptor carries a synonym whose text is empty or whitespace
- **THEN** it is reported as absent and the presentation falls back to the
  metadata name

#### Scenario: Descriptor comment
- **WHEN** a descriptor carries a comment after its synonyms
- **THEN** the resolved object or field carries that comment

#### Scenario: No descriptor
- **WHEN** a physical table resolves without a matching Config descriptor
- **THEN** the object carries no synonym and no comment, and its
  presentation is its metadata name when it has one
