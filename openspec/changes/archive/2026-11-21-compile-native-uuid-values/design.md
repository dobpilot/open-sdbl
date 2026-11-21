# Design

ParameterValue::Uuid(Guid) carries canonical identity; render the existing Guid::to_1c_bytes through SqlDialect::reference_uuid. Parameter kind is ColumnKind::Uuid. Reuse existing driver UUID decoding.

At metadata resolution build a private set of attributes with exactly one descriptor whose declared type is a singleton #fc01b5df-97fe-449b-83d4-218a090e681e without qualifiers. Missing, duplicate, compound and unknown declarations stay unchanged. Queryable fields classify only a single physical binary column of such an attribute as UUID. This index is built once per snapshot, not once per field query.

Convert only raw physical UUID columns. Derived expressions and subquery fields already use the logical UUID domain and must not be converted twice. Existing byte-array parameters remain byte arrays. The new UUID parameter and acquired UUID attribute forms reject SQL Server before execution until measured; existing UUID(reference) is unaffected.

Evidence: open-bsl docs/research/fixtures/uuid-query-parameter-native-1015. Native 8.3.27.2342 equality, different UUID and result type are measured; storage bytes match the existing Guid conversion. The exact original union is replayed after SDK integration.
