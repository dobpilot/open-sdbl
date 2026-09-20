## Why

Row-level filtering today is opt-in *per statement text*: only a statement
that begins `ВЫБРАТЬ РАЗРЕШЕННЫЕ` requests restrictions, and only the
sources that statement names through `ИЗ`, joins, nested queries and
virtual tables become `RestrictionTarget`s. Everything else reads the base
unfiltered — most importantly a dereference, `Т.Контрагент.Наименование`,
which silently joins `Справочник.Контрагенты` with no filter at all.

An application that must guarantee protected reads of an *arbitrary*
query therefore cannot: answering every target of
`Prepared::restriction_request()` is not enough, and a query without the
keyword is not protected at all. `bsl-1c-orm`, in the neighbouring
`open-bsl` repository, needs exactly that guarantee.

## What Changes

- A **restricted mode** SHALL be selectable at preparation, independent of
  the query text: every statement of the batch, every nested query, union
  branch, explicit join, virtual table and temporary-table read is
  filtered whether or not `РАЗРЕШЕННЫЕ` is written. The mode is not
  implemented by inserting the keyword into the source.
- `Prepared` SHALL carry the mode, and no later `compile`/`compile_with`
  call SHALL be able to lower it.
- Every read of a base table SHALL be covered: the restriction request
  SHALL list the implicit reads too — reference dereferences, composite
  references, reference presentations — and a supplied decision SHALL
  actually filter that read. A construct whose safe application is not
  implemented SHALL be refused in restricted mode by a typed diagnostic
  before any SQL is produced.
- A target without an explicit decision SHALL fail compilation. The
  application SHALL answer each target with one of: allowed unfiltered,
  allowed under a condition, or denied. Denial SHALL render a false
  predicate, never an absent filter. No failure path SHALL fall back to an
  unfiltered read.
- The trust boundary of the restriction texts themselves stays where it
  is: a condition is host-supplied and its own reads are not filtered
  recursively. This change states that boundary in the specification and
  keeps user query text out of that path.
- `open-sdbl-db` SHALL turn `Access::Denied` into a denial, and SHALL
  abort preparation on any `RestrictionError` instead of offering a
  partially expanded set.

## Capabilities

### Modified Capabilities

- `query-compilation`: the restricted compilation mode, the explicit
  access decisions it demands, the reads it covers, and the constructs it
  refuses.
- `access-rights`: how the decisions of a user's roles reach the compiler,
  and that an expansion failure is never an unrestricted read.

## Impact

- `src/query.rs` (`PrepareOptions`, `Prepared`), `src/query/core/restrict.rs`
  (`AccessDecision`), `src/query/core/params.rs` (`CompileOptions`),
  `src/query/core/resolve.rs` (catalog state), `src/query/core/codegen/`
  (the base-table read guard and the dereference wrapper).
- `crates/open-sdbl-db/src/access.rs` (decisions instead of conditions).
- New `tests/query_strict_restrictions.rs`; README and
  `docs/query-language-support.md`.
- No new production dependency; the core crate keeps its no-I/O rule.
