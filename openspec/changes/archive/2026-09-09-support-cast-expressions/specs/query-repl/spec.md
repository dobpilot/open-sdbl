## ADDED Requirements

### Requirement: Compile cast expressions
The compiler SHALL accept bilingual `ВЫРАЗИТЬ`/`CAST` with one expression
and a target of `СТРОКА(n)`, `ЧИСЛО(p,s)`, `БУЛЕВО`, `ДАТА`, or a tabular
metadata object. Scalar targets SHALL compile to native conversions on both
providers and report the target as the column kind. A metadata target SHALL
require a reference field, SHALL yield the reference value guarded by the
runtime type discriminator, and MAY be followed by one `.Field` that
dereferences through a type-guarded join reusing the presentation join
cache. Unsupported targets, non-reference arguments, and deeper paths SHALL
fail with positional diagnostics.

#### Scenario: Bounded string
- **WHEN** a query projects `ВЫРАЗИТЬ(Комментарий КАК СТРОКА(500))`
- **THEN** PostgreSQL SQL uses `substring(… from 1 for 500)`, MSSQL SQL uses
  `CONVERT(nvarchar(500), …)`, and the column kind is a string of length 500

#### Scenario: Narrowed dereference
- **WHEN** a query projects `ВЫРАЗИТЬ(Регистратор КАК Документ.Заказ).Дата`
  from a universal reference field
- **THEN** generated SQL joins the document table on `RRRef` with an `RTRef`
  guard and projects its date column

#### Scenario: Fixed target mismatch
- **WHEN** a field references exactly one catalog and the cast names a
  different object
- **THEN** compilation fails with a positional diagnostic

### Requirement: Render boolean predicates on MSSQL
When a boolean field, boolean literal, or boolean cast appears in a predicate
position (`WHERE`, `ON`, or an operand of `AND`, `OR`, `NOT`), generated
T-SQL SHALL compare it with `0x01`/`0x00` so that SQL Server accepts it;
PostgreSQL SQL SHALL keep the bare boolean form.

#### Scenario: Bare boolean field
- **WHEN** a query filters with `ГДЕ Проведен И Код = "A"`
- **THEN** MSSQL SQL contains `([t].[_posted] = 0x01)` and PostgreSQL SQL
  contains the bare column
