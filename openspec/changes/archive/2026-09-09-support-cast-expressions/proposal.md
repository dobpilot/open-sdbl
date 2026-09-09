## Why

Real 1C queries use `ВЫРАЗИТЬ`/`CAST` to bound string lengths, coerce
numbers, and narrow composite references to one type before dereferencing
(`ВЫРАЗИТЬ(Регистратор КАК Документ.Заказ).Дата`). The compiler rejects the
function as unsupported syntax. Separately, a boolean field used directly as
a predicate compiles to `WHERE [t].[_fld]`, which PostgreSQL accepts and SQL
Server rejects because `bit` is not a boolean expression.

## What Changes

- Recognize bilingual `ВЫРАЗИТЬ`/`CAST` as a contextual keyword.
- Compile scalar casts `СТРОКА(n)`/`STRING(n)`, `ЧИСЛО(p,s)`/`NUMBER(p,s)`,
  `БУЛЕВО`/`BOOLEAN`, `ДАТА`/`DATE` on both providers and report the target
  as the column kind.
- Compile reference narrowing `ВЫРАЗИТЬ(<field> КАК <Kind>.<Object>)`: the
  value is the `RRRef` guarded by the `RTRef` discriminator (or a diagnostic
  when a fixed-target field cannot hold the type), and one trailing
  `.<Field>` dereferences through a type-guarded `LEFT JOIN` that reuses the
  presentation join machinery.
- Render boolean fields, boolean literals, and boolean casts in predicate
  positions (`WHERE`, `ON`, `AND`/`OR`/`NOT` operands) as comparisons on
  MSSQL; PostgreSQL output is unchanged.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `sdbl-lexer`: one more bilingual keyword.
- `query-repl`: cast expressions and MSSQL boolean predicates.

## Impact

- Lexer table grows to 48 entries; `Expression` gains a variant.
- Existing MSSQL goldens without boolean predicates are unchanged.
