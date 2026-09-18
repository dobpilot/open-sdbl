## Why

The extra-dimension kinds table of a chart of accounts
(`ПланСчетов.X.ВидыСубконто`, physically `_Acc<N>_ExtDim<M>`) exposed its
standard columns under the schema names only, so `ВидСубконто` and
`ТолькоОбороты` were not found; two demo Бухгалтерия queries read them.
The corpus runner also refused batches that end with `ПОМЕСТИТЬ`.

## What Changes

- `DimKind` SHALL answer to `ВидСубконто` and `TurnoverOnly` to
  `ТолькоОбороты`.
- The corpus runner records a batch that ends with a definition as the
  count over the last table it defines.

## Capabilities

### Modified Capabilities

- `onec-metadata`: standard field aliases.

## Impact

`standard_field_aliases` in `src/query/core/resolve.rs`;
`tests/query_corpus.rs`.
