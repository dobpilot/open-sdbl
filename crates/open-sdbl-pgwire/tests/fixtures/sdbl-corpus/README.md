# SDBL corpus through pgwire and psql

These outcomes cover every entry of the existing `demo`, `unf` and `buh`
corpora under `tests/fixtures/`. Queries are read from those files, not copied
into a second corpus. The shared reader and recorded parameter/session values
live in `tests/support/corpus.rs` and are also used by the compiler corpus test.

The authenticated fake gateway uses production routing, field authorization,
compilation and result encoding. Its test role grants object Read broadly;
unknown field mappings still fail closed. The database engine returns no rows.
**An OK result checks the protocol/authorization/compilation path, not execution
of generated SQL against PostgreSQL/MSSQL or correctness of data results.**

## Results

| Fixture | Attempted | OK | Refused |
| --- | ---: | ---: | ---: |
| demo | 382 | 180 | 202 |
| unf | 708 | 257 | 451 |
| buh | 636 | 204 | 432 |
| Total | 1726 | 641 | 1085 |

| SQLSTATE | Count | Meaning |
| --- | ---: | --- |
| 42501 | 730 | Rights/session metadata could not prove access |
| 0A000 | 198 | Unsupported feature in the gateway's restricted compilation |
| 42P01 | 108 | Missing object, prerequisite temporary table or pruned fixture metadata |
| 42703 | 29 | Field absent from the available metadata |
| 42601 | 12 | Parser refusal of the submitted text |
| 42P02 | 7 | Unbound table-valued parameter |
| 54001 | 1 | Compilation work budget |

Matching a recorded refusal passes the regression test; it does **not** mean
that query is supported. Each TSV row includes the exact error message, source
resource, original text hash and the compiler-corpus classification. For example,
`demo:1` fails on an unsupported Period field mapping, while `demo:4` cannot
read a filter criterion under restricted compilation. Per-entry evidence is
more useful than treating the compiler's much larger unrestricted success
count as gateway compatibility.

## Inputs and limits

- The same native metadata packs and pruned live-schema fixtures as the core
  compiler corpus are loaded. No production database is contacted.
- Scalar parameters are replaced using lexer spans with their recorded SDBL
  date/string/boolean/number literals, or NULL when no value was recorded.
  String contents and comments are never replaced. This is a Simple Query
  input adaptation; NULL literals can have different parsing/typing limitations
  from bound parameters. Separate psql `\bind` tests cover real Extended Query.
- Table-valued parameters have no gateway wire encoding and remain unbound.
  External `__vt_…` prerequisites are not fabricated. Their first actual refusal
  is recorded, which may be authorization before the missing input is reached.
- Raw replay uses one authenticated connection per fixture and `DISCARD ALL`
  between entries. psql uses a fresh connection and `-c` per entry, sending the
  complete batch as one Simple Query instead of splitting it at semicolons.
- psql verification compares SQLSTATE, backend statement count and generated SQL
  hash for every entry. Raw replay also compares result descriptions and errors.
  Internal/protocol errors and loss of ReadyForQuery fail the test immediately.
- The psql child has a 30-second deadline and bounded, concurrently drained
  stdout/stderr. The corpus runs require psql and a loopback listener and are
  explicitly ignored in the default test suite. Baseline completeness and
  original-source hashes are checked by an ordinary fast test.

## Reproduce

From the repository root:

```sh
cargo test -p open-sdbl-pgwire sdbl_corpus_through_gateway -- --ignored --nocapture
cargo test -p open-sdbl-pgwire psql_sdbl_corpus -- --ignored --nocapture
cargo test -p open-sdbl-pgwire tests::psql:: -- --ignored --nocapture
```

`CORPUS_FIXTURE=demo` (or a comma-separated selection) narrows a replay;
without it all 1,726 entries are checked. `OPEN_SDBL_PSQL=/path/to/psql`
chooses the client. Verified with local psql 18.6; the Extended Query test
requires a client that supports `\bind`.

TSV fields are: 1-based entry index, Config source, original-text SHA-256,
SQLSTATE/OK, backend statement count, generated-SQL SHA-256, RowDescription
SHA-256, compiler result class, JSON-escaped error message. Hash input parts
are prefixed with their little-endian u64 byte length. Empty input hashes
as SHA-256 of an empty byte sequence.

After reviewing an intentional outcome change, regenerate explicitly:

```sh
PGWIRE_UPDATE_CORPUS=1 cargo test -p open-sdbl-pgwire sdbl_corpus_through_gateway -- --ignored --nocapture
```

The psql runner cannot record baselines. Review changes to success/refusal
counts and error messages before accepting a new baseline.
