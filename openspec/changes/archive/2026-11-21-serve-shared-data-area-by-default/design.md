# Design: default conditional separators to the shared area

## Problem
`MetadataField::separation.conditional_separation` names a Config constant
that decides whether a common attribute separates. When the session has no
value for that constant, the compiler resolves the separator to
`MissingCondition` and refuses the table. Plain separators have an
empty-value default; conditional ones do not, by design, so the operator
or the session must supply the condition.

## Decision
The gateway, which already binds operator session parameters and forbids
the client from setting any, binds each conditional separator's condition
constant to `false` (the shared, empty data area) when deriving a
connection's session, unless the operator already bound it. This happens
before the session-parameter reads so the `ПараметрыОграниченияДоступа`
register reads cleanly too.

`false` is correct for a base that is not divided into data areas: the
constant is off and all rows carry the empty separator value, which the
compiler's empty-value path matches. For a genuinely separated base the
operator still wins by binding the condition (and the separator value)
explicitly; serving a chosen area of a multi-area base is a later change.

## Implementation
`DbEngine::derive_session` iterates `state.snapshot().separators()`, and
for each `separation.conditional_separation` constant not already present
in the operator's `SessionParameters::separation_condition`, calls
`set_separation_condition(constant, false)`. Pure in-memory; no query.

## Testing
Verified live against `unf` (PostgreSQL, 8.3.27): before, every query
failed with `data separator requires condition constant
cc906f05-…`; after, `ВЫБРАТЬ … ИЗ Справочник.Валюты` returns rows and the
derivation logs no separator warning. The fake-engine gateway tests keep
passing (their fixture has no conditional separator).
