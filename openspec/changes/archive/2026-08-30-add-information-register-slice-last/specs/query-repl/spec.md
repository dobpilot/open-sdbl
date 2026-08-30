## ADDED Requirements

### Requirement: Compile information-register SliceLast sources
The compiler SHALL accept bilingual
`InformationRegister.<name>.SliceLast([period][, condition])` and
`РегистрСведений.<name>.СрезПоследних([period][, condition])` sources.
It SHALL resolve the main table, Period field, Config-declared dimensions, and
data separators through the metadata snapshot. PostgreSQL generation SHALL
select every row at the greatest eligible Period in each dimension/separator
partition. The optional period SHALL be a scalar literal and the optional
condition SHALL use only direct fields and the existing bounded expression
operators. Unsupported or non-information-register use SHALL fail before SQL
execution.

#### Scenario: Current latest slice
- **WHEN** an empty-argument SliceLast source is queried
- **THEN** generated PostgreSQL returns rows at the greatest Period for every
  authoritative dimension and data-separator combination

#### Scenario: Tied latest records
- **WHEN** more than one record in a partition has the greatest eligible Period
- **THEN** every tied record remains in the slice

#### Scenario: Period boundary
- **WHEN** SliceLast receives a scalar period literal
- **THEN** the Period upper bound is applied before greatest-period selection

#### Scenario: Virtual condition precedes slicing
- **WHEN** a condition is passed as the second SliceLast parameter
- **THEN** it filters candidate records before greatest-period selection

#### Scenario: WHERE follows slicing
- **WHEN** an ordinary WHERE follows a SliceLast source
- **THEN** it filters the already selected latest rows and cannot reveal an
  older record

#### Scenario: Joined SliceLast source
- **WHEN** either side of a supported JOIN is an information-register SliceLast
  source
- **THEN** the derived relation participates with the same alias and field
  resolution behavior as its main metadata source

#### Scenario: Invalid SliceLast source
- **WHEN** SliceLast is applied to another metadata kind, a table without
  Period, a parameter period, or a condition containing a reference-property
  dereference
- **THEN** compilation returns a positional diagnostic and no SQL

