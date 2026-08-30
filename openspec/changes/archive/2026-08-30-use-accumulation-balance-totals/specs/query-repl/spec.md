## MODIFIED Requirements

### Requirement: Compile accumulation-register Balance sources
The compiler SHALL accept bilingual
`AccumulationRegister.<name>.Balance([period][, condition])` and
`РегистрНакопления.<name>.Остатки([period][, condition])` sources for
balance registers. It SHALL resolve the register's balance-totals table only
from the same object GUID's `DBNames` `AccumRgT` entry and require that table in
SchemaStorage and the live catalog. It SHALL group totals by Config dimensions
and data separators, merge split totals, expose each Config resource with
`Balance`/`Остаток` suffix aliases, and remove groups whose every balance is
zero. An optional scalar period literal SHALL be an exclusive upper boundary;
historical balances SHALL start from a stored totals anchor and apply only the
bounded signed movement delta. An optional direct dimension/separator condition
SHALL be applied before aggregation.

#### Scenario: Current balances
- **WHEN** Balance is called without a period
- **THEN** only the latest `_AccumRgT*` totals period contributes and split
  rows are merged by dimensions

#### Scenario: Balance at a point
- **WHEN** Balance receives a period literal
- **THEN** a stored totals anchor is combined with active movements so only
  movements strictly before that point affect the result

#### Scenario: Zero balance
- **WHEN** every resource sum for one dimension combination is zero
- **THEN** that combination is absent from the Balance result

#### Scenario: Balance filter placement
- **WHEN** Balance receives a virtual condition and is followed by WHERE
- **THEN** the virtual condition restricts both totals and movement branches
  before aggregation and WHERE filters aggregated balances

#### Scenario: Invalid balance register
- **WHEN** Balance is used on a non-accumulation object, a turnover-only
  register, or a register without a matching declared and live `AccumRgT` table
- **THEN** compilation returns a diagnostic and no SQL
