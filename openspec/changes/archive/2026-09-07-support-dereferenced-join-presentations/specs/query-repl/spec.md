## MODIFIED Requirements

### Requirement: Compile application-defined value presentations
The core compiler SHALL support the reference
`.Представление`/`.Presentation` property, the bilingual
`ПРЕДСТАВЛЕНИЕССЫЛКИ`/`REFPRESENTATION` function, and the bilingual
`ПРЕДСТАВЛЕНИЕ`/`PRESENTATION` function in projections. Before SQL generation,
the core SHALL return one deduplicated batch containing every possible
reference target object ID needed by the query. The application SHALL answer
with fields and a structured presentation template for each target. The core
SHALL validate those plans and compile them to PostgreSQL without accepting raw
SQL or metadata names from the application. In a joined branch, a presentation
MAY consume a supported one-hop dereferenced field; its presentation join SHALL
use the dereference alias as its source and SHALL reuse compatible ancestor
joins.

#### Scenario: Source reference presentation
- **WHEN** a query applies `ПРЕДСТАВЛЕНИЕССЫЛКИ` to the source `Ссылка`
- **THEN** the request contains the source object GUID and the generated SQL
  applies its returned plan to the source row without an unnecessary join

#### Scenario: Fixed reference field presentation
- **WHEN** a reference field has one SchemaStorage target
- **THEN** the request contains that target GUID and generated SQL uses one
  reusable LEFT JOIN to evaluate the target's returned template

#### Scenario: Multiple possible reference targets
- **WHEN** a pure reference field can contain more than one target type
- **THEN** the request contains every target GUID and generated SQL selects the
  corresponding template by the physical RTRef type discriminator

#### Scenario: Universal reference target
- **WHEN** SchemaStorage declares an empty `R` target and a bounded query
  presents that reference
- **THEN** the main SQL returns a typed deferred payload for the projected
  value and retains its predicates and `TOP`/`LIMIT`

#### Scenario: Bounded deferred lookup
- **WHEN** the application resolves deferred payloads from returned rows
- **THEN** it groups them by runtime RTRef object type and uses core-generated
  batch lookup SQL with the validated presentation plan for that object only

#### Scenario: Unknown runtime reference type
- **WHEN** a deferred payload contains an RTRef discriminator absent from the
  metadata snapshot
- **THEN** resolution fails explicitly instead of choosing a table by name or
  rendering the raw binary reference as a presentation

#### Scenario: Scalar REFPRESENTATION
- **WHEN** `ПРЕДСТАВЛЕНИЕССЫЛКИ` receives a non-reference expression
- **THEN** it preserves that expression's value and does not request a plan

#### Scenario: Scalar PRESENTATION
- **WHEN** `ПРЕДСТАВЛЕНИЕ` receives a non-reference expression such as `4`
- **THEN** generated SQL converts it to text and the logical result is `"4"`

#### Scenario: Identifier-only callback protocol
- **WHEN** the application receives a presentation request or returns a plan
- **THEN** objects and custom attributes are identified by real metadata GUIDs
  and standard fields by stable numeric IDs, with no table or field name in the
  protocol

#### Scenario: Safe template
- **WHEN** the application returns a concatenation of field IDs and literal
  text
- **THEN** the core quotes SQL identifiers and literals itself and emits no
  application-provided raw SQL

#### Scenario: Invalid presentation plan
- **WHEN** a plan is missing, references a field outside its target object, or
  has an invalid expression shape
- **THEN** compilation fails with a typed diagnostic before database execution

#### Scenario: Presentation of a dereferenced JOIN field
- **WHEN** a joined projection presents `Ссылка.ДоговорКонтрагента` or
  `ЦФО.Сам_БизнесРегион`
- **THEN** generated SQL first joins the owner of the selected property and
  then joins the property's presentation target from that owner alias

#### Scenario: Reused dereference ancestor
- **WHEN** ordinary projection and presentation require the same first-hop
  dereference
- **THEN** generated SQL contains one shared ancestor join followed by only the
  required presentation joins

#### Scenario: Presentation through joins and unions
- **WHEN** presentation projections occur in supported JOIN, transposed FULL
  JOIN, or compatible UNION branches
- **THEN** each branch retains its validated bindings and compatible output
  shape
