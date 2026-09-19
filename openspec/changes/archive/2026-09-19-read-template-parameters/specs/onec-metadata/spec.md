## ADDED Requirements

### Requirement: Read a value a column stores
The library SHALL read the reference a `ХранилищеЗначения` column holds —
a record starting with `STORHDR`, carrying the sixteen-byte key of the
content and its length — SHALL provide, for both providers, a statement
reading the parts of that content from the platform table `binarydata` in
the order of their offsets, and SHALL decode the concatenated parts into
the serialized value: a ten-byte header, an optional byte-order mark, and
the brace serialization, raw-deflated when the column compresses it. A
column holding anything else SHALL answer that it is no such reference,
without an error.

#### Scenario: A stored map
- **WHEN** a column holds a `STORHDR` reference and the parts of its
  content spell `{"#",<guid>,{N,{{"S","Ключ"},{"S","Значение"}}…}}`
- **THEN** the value decodes and its entries answer `Ключ` with
  `Значение`

#### Scenario: Not a stored value
- **WHEN** the column is empty or holds other bytes
- **THEN** reading the reference answers none
