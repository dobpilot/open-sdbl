## Purpose

Define deterministic, read-only resolution of logical 1C metadata names to the
physical PostgreSQL schema used by an information base.

## ADDED Requirements

### Requirement: Decode platform metadata resources
The library SHALL decode raw-DEFLATE `Params` and `Config` resources, accept
UTF-8 data with or without a byte-order mark, and parse the nested 1C
brace-serialized value format without changing string contents.

#### Scenario: Compressed descriptor with escaped text
- **WHEN** a raw-DEFLATE configuration resource contains a BOM, nested lists,
  and a quoted string with doubled quotes
- **THEN** the library returns the original logical string and value hierarchy

#### Scenario: Malformed resource
- **WHEN** compressed data or brace serialization is truncated or malformed
- **THEN** the library returns a diagnostic instead of partial trusted metadata

### Requirement: Treat DBNames as the authoritative physical-name map
The library SHALL obtain GUID, alias, and numeric database identifiers from
`Params` where `FileName = 'DBNames'`, preserve multiple entries for one GUID,
and SHALL NOT infer a GUID or logical name from a physical table number.

#### Scenario: Main object mapping
- **WHEN** DBNames contains
  `{b56f25d2-72a9-4d80-8998-77ac3097c873,"Reference",2565}`
- **THEN** the resolved main physical table is `_Reference2565`

#### Scenario: Shared field and separator entries
- **WHEN** one GUID has `Fld` and `DataSeparationUse` or
  `DataSeparationHolder` entries
- **THEN** the field number resolves to that GUID and the field is marked as a
  data separator

#### Scenario: Missing authoritative map
- **WHEN** DBNames is absent, empty, compressed incorrectly, or contains no
  valid entries
- **THEN** resolution fails without scanning physical names as a fallback

### Requirement: Resolve supported tabular metadata kinds
The library SHALL map the DBNames aliases `Reference`, `Document`, `Enum`,
`InfoRg`, `AccumRg`, `AccRg`, `CRg`, `Chrc`, `CKinds`, `Acc`, `Const`, `Node`,
`BPr`, `Task`, and `Seq` to their distinct logical kinds and canonical physical
prefixes.

#### Scenario: Ambiguous prefixes
- **WHEN** accounting-register and chart-of-accounts objects are present
- **THEN** `AccRg` resolves to `_AccRg` and `Acc` resolves independently to
  `_Acc`

#### Scenario: Non-tabular descriptor
- **WHEN** a bare-GUID Config descriptor has no recognized tabular DBNames alias
- **THEN** its GUID and human name remain available without inventing a physical
  table

### Requirement: Obtain human names from bare-GUID Config descriptors
The library SHALL resolve metadata names, localized synonyms, and descriptor
markers from descriptors contained in part-zero `Config` resources whose
`FileName` is a bare GUID. A resource MAY contain descriptors for its owner and
for nested metadata such as attributes. Resources named `<guid>.<part>` SHALL
NOT be used as the source of metadata names.

#### Scenario: Object and attribute descriptors
- **WHEN** a bare-GUID resource contains a nested descriptor
  `{1,0,<guid>},"КоррСчет",{2,"ru","Корр. счет"}`
- **THEN** the resolved name is `КоррСчет` and the Russian synonym is
  `Корр. счет`

#### Scenario: Additional resource slot
- **WHEN** only `<guid>.0` contains a matching string
- **THEN** that string is not accepted as the object's metadata name

### Requirement: Parse the authoritative physical schema
The library SHALL parse plaintext `SchemaStorage.CurrentSchema` for
`SchemaID = 0` into physical tables, columns, declared indexes, column type
tags, and reference targets. The parser SHALL preserve canonical 1C spelling
and SHALL NOT require SQL foreign keys.

#### Scenario: Reference column
- **WHEN** SchemaStorage contains a column definition with
  `{"R",...,"Reference35",...}`
- **THEN** the column exposes `Reference35` as its schema reference target

#### Scenario: Stable schema snapshot
- **WHEN** both SchemaStorage and DBSchema exist
- **THEN** SchemaStorage remains the source used to compare with live indexes

### Requirement: Collapse physical columns into logical fields
The library SHALL collapse the physical suffix groups for references,
enumerations, value storage, and compound values into logical fields, remove a
leading underscore from the logical name, and exclude DBNames data-separation
fields when comparing index keys.

#### Scenario: Reference pair
- **WHEN** `_Fld12_TRef` and `_Fld12_RRRef` occur in one table
- **THEN** they resolve to one logical field named `Fld12`

#### Scenario: Recorder reference pair
- **WHEN** `_RecorderTRef` and `_RecorderRRef` occur in one table
- **THEN** they resolve to one logical field named `Recorder`

#### Scenario: Compound value
- **WHEN** columns share a base and use `_TYPE`, `_L`, `_N`, `_T`, `_S`,
  `_RTRef`, and `_RRRef` suffixes
- **THEN** they resolve to one compound logical field with all physical members

### Requirement: Normalize PostgreSQL catalog identifiers at the boundary
The PostgreSQL adapter SHALL compare lowercase catalog identifiers with
canonical SchemaStorage spelling using longest-token-first recasing and SHALL
leave parsed DBNames, Config, and SchemaStorage text unchanged.

#### Scenario: Longest prefix wins
- **WHEN** PostgreSQL reports `_accrg3942` or `_seqb10`
- **THEN** recasing produces `_AccRg3942` or `_SeqB10`, not `_Accrg3942` or
  `_Seqb10`

#### Scenario: Compound suffixes
- **WHEN** PostgreSQL reports `_fld12_rrref` and `_fld12_type`
- **THEN** recasing produces `_Fld12_RRRef` and `_Fld12_TYPE`

### Requirement: Verify against the live PostgreSQL catalog read-only
The CLI SHALL provide `open-sdbl metadata postgres` and use fixed SELECT-only
queries in a read-only PostgreSQL session to read DBNames, bare-GUID Config
descriptors, SchemaStorage, tables, columns, and indexes. It SHALL report
resolved and missing physical objects without extracting, guessing, or printing
1C user passwords.

#### Scenario: Resolved information base
- **WHEN** valid connection options identify a PostgreSQL 1C information base
- **THEN** the command prints each resolved GUID, kind, human name, canonical
  physical name, owner table for fields, and live-catalog status

#### Scenario: PostgreSQL authentication
- **WHEN** PostgreSQL requires a password
- **THEN** the command delegates authentication to `psql` environment or
  password-file mechanisms and does not accept the password as a printed
  command argument

#### Scenario: Read-only enforcement
- **WHEN** the CLI invokes `psql`
- **THEN** it sets the session default to read-only and executes no mutating SQL
