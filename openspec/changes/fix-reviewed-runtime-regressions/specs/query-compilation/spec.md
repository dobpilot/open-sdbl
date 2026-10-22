## ADDED Requirements

### Requirement: Apply scalar equality independently of source and operand shape
The measured PostgreSQL scalar string equality semantics SHALL apply with and
without FROM and with binary expressions as operands. String equality SHALL
ignore trailing ASCII spaces and preserve case; incompatible Number, Boolean
or DateTime operands SHALL produce false with SQL NULL propagation. Existing
contextual CASE selection, non-equality operators and MSSQL behavior SHALL
remain unchanged. Both operands SHALL still undergo authorization and validation.

#### Scenario: Source-free scalar comparison
- **WHEN** a SELECT without FROM compares strings or a string with an incompatible scalar
- **THEN** it uses the same measured equality semantics as a sourced SELECT

#### Scenario: Arithmetic or concatenation operand
- **WHEN** either side of equality is a supported arithmetic or concatenation expression
- **THEN** equality is selected using the complete direct operands and their kinds

### Requirement: Refuse source-free recipes after Undefined information loss
A source-free query SHALL expose no logical projection when a selected typed
scalar expression loses a possible Undefined alternative in SQL. Standalone
Undefined and supported ordinary scalar or SQL NULL projections SHALL retain
their exact recipes. This repair SHALL NOT change their physical SQL.

#### Scenario: Mixed Undefined CASE
- **WHEN** a source-free CASE combines Undefined and a string through SQL NULL
- **THEN** no complete logical recipe is advertised

#### Scenario: Exactly represented scalar
- **WHEN** source-free selections contain ordinary scalars, SQL NULL or standalone Undefined
- **THEN** their complete logical projection remains available
