## ADDED Requirements

### Requirement: Describe logical selected occurrences
Compiled queries SHALL expose an optional complete ordered description of logical selected values for supported projection shapes. Every descriptor SHALL retain the full original logical name and identify final zero-based physical column indices. Repeated selections SHALL retain independent occurrences, without grouping by origin or emitted label. Physical SQL and existing output metadata SHALL remain unchanged.

#### Scenario: Scalar selection
- **WHEN** a supported scalar value is selected
- **THEN** its descriptor identifies exactly its physical output column

#### Scenario: Composite Boolean or String
- **WHEN** one Boolean-or-String value is selected
- **THEN** one descriptor identifies its discriminator and the Boolean and String payload columns with explicit type tags

#### Scenario: Repeated and long aliases
- **WHEN** one field is selected twice with distinct aliases exceeding physical label limits
- **THEN** two ordered descriptors preserve both full aliases and their independent physical indices

### Requirement: Preserve projection through expression and union generation
Available descriptions SHALL retain logical occurrence identity captured before expression members are flattened and SHALL describe the final normalized output of already supported CASE, UNION and derived-source projections. No descriptor SHALL be inferred by parsing emitted SQL, grouping origins, or reconstructing aliases from truncated labels.

#### Scenario: Composite CASE
- **WHEN** a supported CASE has Boolean and String alternatives
- **THEN** its emitted members describe one logical occurrence

#### Scenario: UNION normalization
- **WHEN** supported UNION branches require spreading or widening their values
- **THEN** descriptors use first-branch logical names and final normalized output indices and roles

### Requirement: Refuse incomplete logical recipes
A query whose complete exact mapping is unavailable SHALL expose no logical recipe rather than a partial or guessed description. Unknown or ambiguous tag-to-payload mappings and unimplemented derived, nested, totals or deferred transformations SHALL remain unavailable without changing accepted SQL or existing physical APIs. Service columns SHALL NOT become logical selected values. Cell decoding and runtime values SHALL remain outside the core library.

#### Scenario: Ambiguous discriminator tags
- **WHEN** different emitted payload members share a tag that cannot distinguish their active member
- **THEN** no logical recipe is exposed

#### Scenario: Unimplemented transformed result
- **WHEN** a nested, totals or deferred result cannot yet retain an exact complete recipe
- **THEN** the recipe is unavailable while existing result metadata remains unchanged
