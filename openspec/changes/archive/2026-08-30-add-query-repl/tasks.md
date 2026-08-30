## 1. Core query model

- [x] 1.1 Add metadata source lookup with Russian/English kind names and exact authoritative table resolution.
- [x] 1.2 Build queryable logical fields from live physical columns, nested Config descriptors, and standard bilingual names.
- [x] 1.3 Add positional errors and tests for missing, ambiguous, and unsupported sources/fields.

## 2. Bounded compiler

- [x] 2.1 Parse the documented single-source SELECT grammar from existing lexer tokens.
- [x] 2.2 Generate quoted PostgreSQL SELECT statements with text projections, TOP/LIMIT, predicates, and ordering.
- [x] 2.3 Test Russian/English queries, custom fields, wildcard/compound projections, quoting, and rejection of unsupported syntax.

## 3. REPL application

- [x] 3.1 Refactor the tokio-postgres connection lifecycle for metadata refresh and repeated read-only query transactions.
- [x] 3.2 Add asynchronous multiline REPL input, `\help`, `\q`, `\refresh`, recoverable diagnostics, and row rendering.
- [x] 3.3 Implement `\dt`, `\di`, and `\d <metadata-name>` from the resolved snapshot with stable table output.

## 4. Verification

- [x] 4.1 Add piped-input CLI tests for help, meta commands, query errors, multiline termination, and clean exit.
- [x] 4.2 Run the REPL against `192.168.166.15/test` and execute discovery plus a generated query for the conformance catalog.
- [x] 4.3 Update README/help and run all workspace quality gates plus strict OpenSpec validation.
