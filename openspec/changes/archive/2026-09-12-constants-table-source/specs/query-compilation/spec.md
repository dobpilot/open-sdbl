## ADDED Requirements

### Requirement: Compile the constants table source
The compiler SHALL accept `Константы`/`Constants` as a source whose
fields are the names of every constant with a live `_Const<N>` table,
typed as the single-constant source types them, with no standard fields.
The source SHALL render as a derived table reading only the constants the
statement references (`*` references all): a `UNION ALL` of one `SELECT`
per constant projecting its physical columns and `CAST(NULL AS <catalog
type>)` for the columns of the other constants, aggregated with `MAX` per
column and without `GROUP BY`, so the result is exactly one row; a
statement referencing no constant reads a one-row stand-in. Separator predicates SHALL
apply inside every branch; a referenced constant whose separator is
disabled for the statement SHALL be an `UnsupportedFeature` diagnostic at
the source naming the constant. `Константа.Имя` SHALL keep its rendering.

#### Scenario: Two constants
- **WHEN** `ВЫБРАТЬ К.А, К.Б ИЗ Константы КАК К` compiles
- **THEN** the source is `(SELECT MAX(u."_Fld<A>") …, MAX(u."_Fld<B>") …
  FROM (SELECT t."_Fld<A>", CAST(NULL AS <type of B>) FROM "_Const<A>" AS
  t UNION ALL SELECT CAST(NULL AS <type of A>), t."_Fld<B>" FROM
  "_Const<B>" AS t) AS u) AS "К"` and constants the statement does not
  reference are absent

#### Scenario: Unwritten constant
- **WHEN** a referenced constant's table has no rows
- **THEN** the query still returns one row with `NULL` in that column

#### Scenario: Reference constant dereference
- **WHEN** `ВЫБРАТЬ Константы.Организация.Наименование ИЗ Константы`
  compiles
- **THEN** the derived table projects the `RRef` and `_TYPE` columns and
  the dereference renders as a `LEFT JOIN` on them

#### Scenario: Disabled separator
- **WHEN** a referenced constant table declares a separator column and the
  separator is disabled for the statement
- **THEN** compilation fails with an `UnsupportedFeature` diagnostic naming
  the constant
