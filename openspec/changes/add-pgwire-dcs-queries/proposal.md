## Why

Clients can retrieve a report's data composition schema through pgwire.
They need to read the names and query texts of its data sets as SQL rows.

## What Changes

- Add `SELECT * FROM onec_dcs_queries(xml)` with two text columns,
  `name` and `query`.
- Accept schema XML from a literal, a bound text parameter, or a nested
  service call such as `onec_show_metadata`.

## Capabilities

### Modified Capabilities

- `pgwire-gateway`: extract query data sets from 1C data composition schemas.

## Impact

The pgwire crate reuses its XML parser and service call grammar. An
iterative tokenizer bounds client XML before tree parsing. Backing
database queries remain unchanged.
