## MODIFIED Requirements

### Requirement: Decode object headers and their links
The library SHALL return UUID and name for each recognized object header.
Let `<fields>` be `<name>,<synonym>,<comment>` and
`<tail>` be `<belonging>,<n>,(<guid>,<tag>)×n`.
- Version 3: `{3,{1,0,<id>},<fields>,<tail>,<extends>,<x>}`.
- Version 1: `{1,{0,0,<id>},<fields>,<tail>}`.

Belonging SHALL require exactly `n` pairs and use `0` for Own and `1`
for Adopted. Version 1 pairs SHALL contain a GUID and a scalar tag.
Only a version 3 adopted header with a non-nil `<extends>` SHALL yield
a link.

#### Scenario: A version 1 own header
- **WHEN** a version 1 own header has exactly the property pairs it counts, including zero pairs
- **THEN** the library returns Own and no link

#### Scenario: An adopted catalog and its attribute
- **WHEN** the description of `СтавкиНДС` in `Расширение1` is decoded
- **THEN** the catalog is adopted and extends
  `6a386de9-b78e-47cb-a7e8-864b52022b24`, its adopted attribute `Ставка`
  extends `9a871782-6f75-4078-a8f2-a33d9dcb8c7f`, and its own attribute
  `Расш1_Реквизит1` is own with no link

#### Scenario: Adopted by name
- **WHEN** an adopted object carries the nil identifier as `<extends>`
- **THEN** it is answered as adopted with no link

#### Scenario: An unfamiliar tail
- **WHEN** a header's tail is shorter than its pair count says
- **THEN** belonging and link are unknown, and the identifier and name are
  still answered
