## ADDED Requirements

### Requirement: Compile conditional expressions
The compiler SHALL accept `ВЫБОР КОГДА <predicate> ТОГДА <value> …
[ИНАЧЕ <value>] КОНЕЦ` / `CASE WHEN … THEN … [ELSE …] END` wherever a scalar
expression is allowed and SHALL render it as SQL `CASE` on both providers,
compiling each condition as a predicate. The column kind SHALL be the first
non-wildcard branch kind and every branch kind SHALL be compatible with it,
otherwise compilation SHALL fail with a positional diagnostic. Reference
branches with different targets or widths SHALL be widened to one
runtime-typed payload whose targets are the union of the branch targets. An absent `ИНАЧЕ` SHALL
yield `NULL`.

#### Scenario: Conditional projection
- **WHEN** a query projects `ВЫБОР КОГДА Проведен ТОГДА "Да" ИНАЧЕ "Нет" КОНЕЦ`
- **THEN** both dialects emit `CASE WHEN … THEN … ELSE … END`, MSSQL compares
  the boolean field with `0x01` in the condition, and the column kind is string

#### Scenario: Conditional predicate on MSSQL
- **WHEN** `ГДЕ ВЫБОР КОГДА Сумма > 0 ТОГДА ИСТИНА ИНАЧЕ ЛОЖЬ КОНЕЦ` is
  compiled for MSSQL
- **THEN** the generated predicate compares the `CASE` expression with `0x01`

#### Scenario: References to different objects
- **WHEN** one branch yields a document reference and another a catalog
  reference
- **THEN** both branches are rendered as `RTRef ‖ RRRef` payloads and the
  column kind is a runtime-typed reference targeting both objects

#### Scenario: Incompatible branches
- **WHEN** one branch yields a number and another a string
- **THEN** compilation fails with a positional diagnostic at the second branch

### Requirement: Compile default-value expressions
The compiler SHALL accept bilingual `ЕСТЬNULL(<value>, <fallback>)` /
`ISNULL(…)` and SHALL render it as `COALESCE(value, fallback)` on both
providers. The column kind SHALL be the value's kind, or the fallback's kind
when the value is the `NULL` literal, and the two kinds SHALL be compatible;
reference operands SHALL be widened by the same rule as `ВЫБОР` branches.

#### Scenario: Default after an outer join
- **WHEN** a query projects `ЕСТЬNULL(Остатки.КоличествоОстаток, 0)` over a
  left join
- **THEN** generated SQL contains `COALESCE(…, 0)` and the column kind is
  number

#### Scenario: Date default on MSSQL
- **WHEN** `ЕСТЬNULL(Дата, ДАТАВРЕМЯ(1, 1, 1))` is projected with a non-zero
  year offset
- **THEN** the `DATEADD(year, -offset, …)` correction wraps the whole
  `COALESCE` exactly once

### Requirement: Compile pattern predicates
The compiler SHALL accept `<value> [НЕ] ПОДОБНО <pattern> [СПЕЦСИМВОЛ
<escape>]` / `[NOT] LIKE … [ESCAPE …]` in predicate positions and SHALL
render `LIKE` with the pattern passed through unchanged on both providers,
adding `ESCAPE` when supplied and `NOT` when negated. The operands SHALL have
the string kind or a wildcard kind. Using the operator as a projected value
or with non-string operands SHALL fail with a positional diagnostic.

#### Scenario: Bracket class pattern
- **WHEN** a query filters with `Код ПОДОБНО "[0-9]%"`
- **THEN** PostgreSQL and MSSQL SQL both contain `LIKE` with the literal
  pattern `[0-9]%` and no rewritten pattern

#### Scenario: Negated pattern with an escape character
- **WHEN** a query filters with `Наименование НЕ ПОДОБНО "%\_%" СПЕЦСИМВОЛ "\"`
- **THEN** generated SQL wraps the `LIKE … ESCAPE …` predicate in `NOT (…)`

### Requirement: Aggregate arbitrary scalar expressions
`СУММА`, `МИНИМУМ`, `МАКСИМУМ`, and `КОЛИЧЕСТВО([РАЗЛИЧНЫЕ] …)` SHALL accept
any scalar expression as their argument. `СУММА` and `КОЛИЧЕСТВО` SHALL
report a number kind; `МИНИМУМ`/`МАКСИМУМ` SHALL report the argument's kind
and SHALL aggregate the payload of a reference expression on both providers.
Aggregates nested in aggregates SHALL fail with a positional diagnostic.

#### Scenario: Conditional sum
- **WHEN** a query projects `СУММА(ВЫБОР КОГДА Вид = ЗНАЧЕНИЕ(…) ТОГДА Сумма ИНАЧЕ 0 КОНЕЦ)`
- **THEN** generated SQL contains `SUM(CASE WHEN … END)` and the column kind
  is number

#### Scenario: Distinct count of an expression
- **WHEN** a query projects `КОЛИЧЕСТВО(РАЗЛИЧНЫЕ НАЧАЛОПЕРИОДА(Дата, МЕСЯЦ))`
- **THEN** generated SQL contains `COUNT(DISTINCT …)` over the period
  expression
