## Why

1C orders by arbitrary expressions: `УПОРЯДОЧИТЬ ПО ВЫБОР … КОНЕЦ`,
`УПОРЯДОЧИТЬ ПО ТИПЗНАЧЕНИЯ(Т.Объект)`, `УПОРЯДОЧИТЬ ПО Цена * 2`. The
parser accepts only a field path or a projection alias there, so such a
query fails with `unsupported query syntax starting at "("`, and the
capability table still marks `УПОРЯДОЧИТЬ ПО` partial for that reason.

## What Changes

- The parser SHALL accept any supported expression as an ordering key,
  keeping the field path and projection alias forms exactly as they are.
- A plain branch SHALL order by the compiled expression. A branch whose
  ordering is positional (joined, grouped, or a union) SHALL keep
  requiring a projected column or an alias and SHALL report the same
  diagnostic for an expression key.
- A statement with `ИТОГИ` SHALL project an expression key as a hidden
  column, as it already does for alias keys.
- Ordering by a type value SHALL follow the encoded type, which is the
  order the platform produces: undefined, then the primitive types, then
  references by table number.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `query-repl`: ordering keys may be expressions.

## Impact

- `src/query/core/ast.rs`, `parser.rs`, `codegen/select.rs`,
  `codegen/sources.rs`, `codegen/orchestrate.rs`; README and
  `docs/query-language-support.md`.
