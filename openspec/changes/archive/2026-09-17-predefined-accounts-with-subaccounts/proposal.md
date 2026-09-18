## Why

`ЗНАЧЕНИЕ(ПланСчетов.Хозрасчетный.Касса)` failed on the demo
Бухгалтерия base while `РасчетныеСчета` resolved: a row of the `.9`
resource whose account has subaccounts carries their rows in one more
trailing element, and the reader demanded the exact leaf-row length.

## What Changes

- The reader of a `.9` (and `.1c`, `.7`) predefined table SHALL accept a
  row longer than its column count plus the header and trailer, reading
  the name the same way; the nested rows keep being visited.

## Capabilities

### Modified Capabilities

- `onec-metadata`: predefined accounts with subaccounts.

## Impact

`project_predefined_value` in `src/metadata/config.rs`.
