# Design — restricted compilation mode

## Context

`РАЗРЕШЕННЫЕ` is a property of the *query text*. The compiler turns it into
one boolean on `CompilationCatalog` (`restricting`), consulted by exactly
one call site — `resolve_join_source`, through
`CompilationCatalog::restriction_for` — which wraps a plain source in
`(SELECT … FROM <table> AS "__restricted" WHERE <condition>)`.

Everything that reads a base table without going through that call site is
unfiltered today:

| Read | Where it is built |
|---|---|
| Reference dereference (`Т.Контрагент.Наименование`) | `context.rs`, `JoinPlan` pushed after `compile_live_relation` |
| A hop of a composite reference | same, one `JoinPlan` per candidate type |
| Reference presentation (`ПредставлениеСсылки`) | same |
| `В ИЕРАРХИИ` and `ИТОГИ` hierarchy descents | `totals.rs`, a recursive CTE over `_IDRRef`/`_ParentIDRRef` |
| Filter criterion (`КритерийОтбора`) | `select.rs` |
| `Константы` | `constants.rs` |
| A temporary table | `temp_tables.rs` |

An application answering every entry of `restriction_request()` therefore
still emits unfiltered reads. This change makes protection a property of
the *compilation*, not of the text.

## Decisions

### 1. The mode belongs to preparation

`RestrictionMode::{Statement, Restricted}` is chosen when a query is
prepared and stored on `Prepared`. `Statement` — the default — is exactly
today's behaviour. `Restricted` makes every statement of the batch behave
as if it carried `РАЗРЕШЕННЫЕ`, without touching the source text or the
AST: `compile_statement` arms the catalog from the mode rather than from
`query.allowed.is_some()`.

The mode is reachable only through `Prepared`, added as
`QueryCompiler::prepare_with_options(source, &PrepareOptions)`;
`prepare` and `prepare_with` keep their signatures and their default.
`Prepared::compile`, `compile_with` and `compile_batch` pass the stored
mode down, so no `CompileOptions` value can lower it — `CompileOptions`
carries no mode field at all, which is what makes downgrading
unexpressible rather than merely discouraged.

The one-shot `QueryCompiler::compile`/`compile_with` path stays
unrestricted. It has no request phase, so it cannot collect the decisions
the restricted mode requires; a consumer that needs protection goes
prepare → decisions → compile. This is stated rather than silently
implied.

### 2. Decisions, not optional restrictions

`AccessRestriction` cannot say "allowed, no filter" — its absence means
both "allowed" and "the application said nothing". Restricted mode needs
those distinguished, so the application answers with

```rust
pub enum AccessDecision {
    Unrestricted(RestrictionTarget),
    Restricted(AccessRestriction),
    Denied(RestrictionTarget),
}
```

supplied through `CompileOptions::decisions`. In restricted mode every
target of the request must have exactly one decision; a target without one
is a `Restriction` diagnostic naming the object and, when the target is a
tabular section, its name. `Denied` renders the same derived-table wrapper
with the predicate the dialect writes for a false literal, so a denial is
a filter that admits no row — never an omitted filter.

`CompileOptions::restrictions` stays, means `Restricted` for the targets
it names, and remains sufficient for `Statement` mode, where an unnamed
target is still "no filter" as it is today.

### 3. Coverage is default-deny, not best-effort

A guarantee that rests on remembering to filter each new construct is not
a guarantee. Restricted mode therefore refuses by default: every place
that turns a metadata object into a physical relation asks the catalog for
permission, and the catalog answers only for reads that are wired through
the restriction path. Everything else raises
`QueryDiagnosticKind::UnsupportedFeature` positioned at the construct,
before any SQL exists.

Two reads move from "unfiltered" to "filtered" in this change:

- **Plain sources**, including joins, nested queries, union branches,
  tabular sections, slices, balances, turnovers and accounting tables —
  already wired; restricted mode only arms them unconditionally.
- **Reference joins** — the four `JoinPlan` construction sites share one
  shape: a concrete `target_object`, a relation from
  `compile_live_relation`, and an alias. They gain the same wrapper the
  plain source uses, and register the target in the request. One mechanism
  covers plain dereferences, each candidate hop of a composite reference,
  and presentation joins, because all three are `JoinPlan`s.

The rest are refused in restricted mode in this change: hierarchy descents
(`В ИЕРАРХИИ`, `ИТОГИ … ПО … ИЕРАРХИЯ`), filter criteria, the `Константы`
source, a nested tabular-section projection, a deferred reference
presentation, and a temporary table whose definition was not compiled in
restricted mode. Each refusal is a test, not a gap in the test suite.

The last two deserve a word: their rows are fetched by a *second* query
the caller issues — the nested-section query of `CompiledQuery::nested`
and the batch of `compile_presentation_lookup`. That second query is not
this compilation, so its reads carry none of its decisions.
`compile_presentation_lookup` itself has no restricted mode and is
documented as unfiltered; the restricted mode simply never produces work
for it.

A document journal turned out not to belong on that list: it resolves
through the ordinary source path, so it is requested and filtered like any
other table, and it has a test that says so.

A temporary table *defined inside the restricted batch* is allowed,
because its defining statement was itself filtered. `TempTable` records
the mode it was compiled under so that a manager filled by an earlier
unrestricted batch cannot be read from a restricted one.

### 4. Why a dereference is filtered, not refused

Wrapping the join target hides the attributes of rows the decision
excludes; the `LEFT JOIN` then contributes `NULL`s, exactly as it does for
a reference that points at a deleted row. The reference value itself stays
visible, but it was already a column of the base row the application
allowed. This is a security decision of this library, taken because it is
the containment the wrapper can prove — it is **not** presented as
measured 1C platform behaviour, which this repository states only where it
has been measured against a real base.

### 5. Trust boundary of the restriction texts

A restriction condition is **host input**, not user input. It arrives from
the roles of the configuration through `open_sdbl::access`, or directly
from the embedding application; the person running the query cannot write
it. The compiler already separates the two paths, and this change keeps
the separation and names it:

- `CompilationCatalog::in_restriction` compiles a condition with query
  parameters hidden (session parameters only) and with `restricting`
  cleared, so the tables the condition itself reads — the access-key
  registers of the Standard Subsystems Library, for instance — are read
  unfiltered.
- That is deliberate and stays unchanged. Filtering a condition's own
  reads recursively would require the access keys to be readable under the
  very restriction they are computing, which is circular: the usual
  library templates would stop matching any row and the result would be a
  silent denial of everything.
- Nothing from the user's query enters that path. The user's text is
  parsed by `Parser::parse`, the condition by `Parser::parse_restriction`,
  and a condition never receives query parameter values. Restricted mode
  does not widen what a condition may read.

The consequence the specification states plainly: the host is responsible
for the conditions it supplies. A condition that reads a table the user
may not read is a decision of the host, and the library will not
second-guess it.

### 6. `open-sdbl-db` and the roles of a user

`user_restrictions` today answers with `AccessRestriction`s for the
targets it could expand and leaves the others alone — which, under
restricted mode, would be a missing decision, and under the old mode an
unfiltered read. It becomes a producer of `AccessDecision`s:

- `Access::Unrestricted` → `AccessDecision::Unrestricted`;
- `Access::Restricted` → `AccessDecision::Restricted` with the expanded
  condition;
- `Access::Denied` → `AccessDecision::Denied`;
- no current user, an unread role, or any `RestrictionError` → the whole
  answer fails. A partially expanded set is never offered as sufficient,
  and missing data is never read as permission.

Authenticating the user stays the host's business; this library gains no
notion of a session or a login.

## Risks

- The guard touches every base-table read. A path missed in the audit
  would be a silent hole, so the audit is anchored on the functions that
  produce a physical relation, and each refusal carries a test.
- Filtering dereferences changes the SQL of restricted-mode queries only;
  `Statement` mode keeps byte-identical output, which the existing tests
  already pin.

## No new dependencies

The change is confined to the core crate and `open-sdbl-db`; neither gains
a dependency.
