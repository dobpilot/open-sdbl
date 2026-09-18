## MODIFIED Requirements

### Requirement: Acquire predefined-value metadata

The application adapters SHALL load assembled bare-GUID Config resources,
assembled `<guid>.1c` resources and assembled `<guid>.9` resources. The
core SHALL decode only verified predefined-value rows — `{2, <index>,
<column count>, {"#", <type>, {1, <guid>}}, …}` with the name in the
first string column — and associate every value with the GUID encoded in
its file name and with the resource kind it came from. Resolution SHALL
keep `.9` values for charts of accounts only and `.1c` values for the
other kinds only. Other Config suffixes SHALL remain excluded.

#### Scenario: Catalog predefined values
- **WHEN** `<catalog-guid>.1c` contains verified predefined rows
- **THEN** each exact symbolic name and stable GUID is associated with that
  catalog without reading catalog business rows

#### Scenario: Predefined accounts
- **WHEN** `<chart-guid>.9` contains the rows measured on the UNF chart
  `Управленческий`
- **THEN** each account's symbolic name and GUID is associated with the
  chart, and a `.9` resource of an object that is not a chart of
  accounts yields no value

#### Scenario: Unrelated suffix resource
- **WHEN** a Config file has a suffix other than `.1c` or `.9`
- **THEN** predefined-value decoding returns no values and does not interpret
  the payload as metadata
