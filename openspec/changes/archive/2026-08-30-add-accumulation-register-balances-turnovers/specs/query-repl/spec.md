## ADDED Requirements

### Requirement: Compile accumulation-register Balance sources
The compiler SHALL accept bilingual
`AccumulationRegister.<name>.Balance([period][, condition])` and
`РегистрНакопления.<name>.Остатки([period][, condition])` sources for
balance registers. It SHALL group active movement rows by Config dimensions and
data separators, apply movement direction to Config resources, expose each
resource with `Balance`/`Остаток` suffix aliases, and remove groups whose
every balance is zero. An optional scalar period literal SHALL be an exclusive
upper boundary and an optional direct dimension/separator condition SHALL be
applied before aggregation.

#### Scenario: Current balances
- **WHEN** Balance is called without a period
- **THEN** every active movement contributes through its receipt/expense sign
  to a dimension-grouped current balance

#### Scenario: Balance at a point
- **WHEN** Balance receives a period literal
- **THEN** only active movements strictly before that point contribute

#### Scenario: Zero balance
- **WHEN** every resource sum for one dimension combination is zero
- **THEN** that combination is absent from the Balance result

#### Scenario: Balance filter placement
- **WHEN** Balance receives a virtual condition and is followed by WHERE
- **THEN** the virtual condition restricts movement rows before aggregation and
  WHERE filters aggregated balances

#### Scenario: Invalid balance register
- **WHEN** Balance is used on a non-accumulation object or a turnover-only
  accumulation register
- **THEN** compilation returns a diagnostic and no SQL

### Requirement: Compile accumulation-register Turnovers sources
The compiler SHALL accept bilingual
`AccumulationRegister.<name>.Turnovers([begin][, end][, periodicity][,
condition])` and the corresponding `РегистрНакопления.<name>.Обороты`
source for balance and turnover-only registers. It SHALL group active movement
rows by Config dimensions and data separators and expose Config resources with
`Turnover`/`Оборот` suffix aliases. A balance register SHALL apply movement
direction, while a turnover-only register SHALL sum stored resource values. The
optional scalar begin and end literals SHALL define a half-open interval. The
initial bounded subset SHALL require the periodicity slot to be omitted and
SHALL accept a direct dimension/separator condition in the fourth slot.

#### Scenario: All-time turnovers
- **WHEN** Turnovers is called without arguments
- **THEN** active resource movements are aggregated by dimensions

#### Scenario: Bounded turnovers
- **WHEN** begin and end literals are provided
- **THEN** generated PostgreSQL applies `Period >= begin` and `Period < end`
  before aggregation

#### Scenario: Turnover filter
- **WHEN** the fourth condition parameter is provided with an omitted
  periodicity slot
- **THEN** it restricts direct dimension/separator fields before aggregation

#### Scenario: Joined aggregate source
- **WHEN** Balance or Turnovers participates in a supported JOIN
- **THEN** its derived relation retains ordinary alias, reference-property, and
  outer-filter behavior

#### Scenario: Unsupported periodicity
- **WHEN** the third Turnovers parameter is nonempty
- **THEN** compilation reports unsupported periodic grouping and emits no SQL

