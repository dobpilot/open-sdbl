## ADDED Requirements

### Requirement: Read query data sets from data composition schema XML
The gateway SHALL support `SELECT * FROM onec_dcs_queries(xml)` for
authenticated clients and return two columns, `name text` and `query text`.
Each data set with a direct `query` element SHALL produce one row in document
order. The gateway SHALL decode the query text from XML, preserve its
whitespace and line breaks, and SHALL NOT execute it.

#### Scenario: Extract queries from a report's main schema
- **WHEN** a client passes `onec_show_metadata(path)` into `onec_dcs_queries`
  and the XML contains several query data sets and a data source
- **THEN** it receives a row per query data set with the corresponding
  name and query text, including any queries inside a union
- **AND** a data set without a name has SQL NULL in the `name` column

#### Scenario: Recognize both XML formats
- **WHEN** the argument is a Designer export or a native schema under
  `SchemaFile` with the 1C schema namespace
- **THEN** the gateway finds the query data sets independently of the XML prefix

#### Scenario: Missing schema or no query data sets
- **WHEN** the argument is SQL NULL or a valid schema has no query data sets
- **THEN** the result has two text columns and zero rows

#### Scenario: Invalid input
- **WHEN** the function receives malformed XML or a document with a DTD
- **THEN** it returns SQLSTATE `2200M`
- **WHEN** a well-formed document has no recognized schema root
- **THEN** it returns SQLSTATE `22023`

### Requirement: Serve data composition query extraction through both protocols
The gateway SHALL permit XML literals and composed text calls in simple
and extended queries, and text parameters in extended queries. It SHALL
describe both text columns before execution and honor the requested
result formats and portal row limits. XML input, the parsed document and
buffered rows SHALL be bounded. An XML argument within the resource size
limit SHALL be allowed to exceed the text path argument limit.

#### Scenario: Prepare with a text parameter
- **WHEN** a client prepares `SELECT * FROM onec_dcs_queries($1)`
- **THEN** the gateway describes a text parameter and both result columns
- **WHEN** the client executes this statement with a valid schema larger
  than the metadata path limit and within XML and protocol limits
- **THEN** it receives the query data sets

#### Scenario: Schema exceeds XML parsing limits
- **WHEN** client XML exceeds the supported depth or another parser limit
- **THEN** the gateway returns SQLSTATE `54000` for the statement
