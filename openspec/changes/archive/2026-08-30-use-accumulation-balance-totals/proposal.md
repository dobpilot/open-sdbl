# Change: Read accumulation balances from stored totals

## Why

`Balance` currently scans and aggregates every active movement. A 1C balance
register already maintains an authoritative `_AccumRgT*` totals table, and the
standard current-balance path must use it instead of rebuilding all history.

## What changes

- Resolve the balance-totals table only through the register GUID's
  `DBNames` `AccumRgT` entry and verify it against SchemaStorage/live catalogs.
- Read current balances directly from the latest stored totals period and
  merge split total rows by dimensions.
- For a historical boundary, start from a stored totals anchor and apply only
  the required signed movement delta.
- Keep `Turnovers` on movement rows and fail before SQL generation when the
  required balance-totals metadata or physical table is unavailable.
