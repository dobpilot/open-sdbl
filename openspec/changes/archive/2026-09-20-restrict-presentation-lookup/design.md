# Design — filtering the presentation lookup

## Context

A presentation of a reference whose target the compiler cannot fix at
compile time is *deferred*: the statement projects the reference payload
and reports a `PresentationRequest`, and the application later calls
`compile_presentation_lookup` with a plan and a batch of references. That
second statement reads the target table directly.

`RestrictionMode::Restricted` refuses to produce such work, because the
second statement carries none of the first one's decisions.

## Decisions

### 1. The lookup belongs to the prepared query that produced it

The references come from rows the user was allowed to read, and the
question the lookup asks — may this user read the *target* — is the same
question the statement asked about its own sources. So the lookup is
compiled from `Prepared`, which already carries the mode:

```rust
Prepared::compile_presentation_lookup(&self, snapshot, plan, references, options)
```

It applies `self.mode` and the decisions in `options`, exactly as
`compile_with` does. `QueryCompiler::compile_presentation_lookup` keeps
its signature and its unfiltered behaviour, and its documentation says so
plainly, because an application that is not in the restricted mode has
nothing to supply.

### 2. The target is a source like any other

The lookup renders `FROM <table> AS "__presentation_target"`. With a
decision it renders
`FROM (SELECT … FROM <table> AS "__restricted" WHERE <condition>) AS
"__presentation_target"` — the same wrapper `compile_source_relation`
builds, so the condition language, the dereferences it may use, and the
session-parameter rules are the ones already specified.

### 3. Exclusion is silence, not an error

A denied or filtered-out reference simply matches no row, so the lookup
returns nothing for it. The application already has to handle a reference
whose row it did not get — a deleted object does the same — so a closed
object presents as empty without a new failure mode. This is the right
semantics rather than a convenient one: the reference itself was in a row
the user could read, so refusing the whole batch would deny data the user
is entitled to.

### 4. Lifting the refusal

With the lookup filterable, `Restricted` stops refusing a deferred
presentation. The refusal for constructs whose filtering is *not*
implemented stays exactly as it is; only the deferred-presentation arm
goes.

The remaining sharp edge is stated rather than hidden: a caller that
resolves a restricted query's deferred presentations through the
unrestricted entry point reads unfiltered. The restricted entry point is
the one on `Prepared`, and that is where the documentation points.

## No new dependencies

Confined to the core crate.
