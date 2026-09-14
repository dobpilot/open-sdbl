## Why

A subquery of a predicate may read the row of the enclosing statement:
`ГДЕ ЛОЖЬ В (ВЫБРАТЬ ПЕРВЫЕ 1 ЛОЖЬ ИЗ РегистрСведений.X КАК Р ГДЕ
Р.Объект = Файлы.Ссылка)` is how real queries ask «does such a record
exist». The compiler refused the outer qualifier, and five demo queries
stop there. The platform answers it, checked on the probe base.

Two more names were missing: the declaration order of an enumeration
value, `Перечисление.X.Порядок`, which the platform answers as a
zero-based number.

## What Changes

- The sources of the enclosing statement SHALL be visible inside a
  subquery of a predicate, by their qualifier, and SHALL render as the
  outer alias.
- They SHALL stay invisible to an unqualified name and to a derived
  source, which SQL evaluates before the outer row exists.
- `Порядок` / `Order` SHALL name the `EnumOrder` column of an
  enumeration.

## Capabilities

### Modified Capabilities

- `query-compilation`: correlated subqueries and the enumeration order.

## Impact

- `src/query/core/codegen/context.rs`, `select.rs`, `orchestrate.rs`,
  `expression.rs`, `src/query/core/resolve.rs`;
  `tests/query_compile.rs`; `tests/fixtures/demo/expected.jsonl`;
  `docs/query-language-support.md`.
