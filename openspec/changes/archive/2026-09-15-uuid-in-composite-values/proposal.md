## Why

`ВЫБОР КОГДА … ТОГДА УНИКАЛЬНЫЙИДЕНТИФИКАТОР(Т.Ссылка) ИНАЧЕ Т.Клиент
КОНЕЦ` is refused with "expression kinds differ", and a corpus query of a
real configuration stops there. The platform accepts it: measured on the
probe base against 8.3.27, it answers the identifier branch with binary
data and the reference branch with the reference, and `ТИПЗНАЧЕНИЯ` of the
whole value answers `Null`, because the platform does not classify a
unique identifier as a stored type.

The compiler refuses because a unique identifier has no member in the
composite layout: the platform stores such a value over
`_TYPE/_L/_N/_T/_S/_RTRef/_RRRef` and none of them is binary — the value
exists only in a result, never in a table.

## What Changes

- Give a unique identifier the member `_U` and raw bytes the member `_B`
  of a composite value, both with the `Null` type tag the platform
  answers, so a `ВЫБОР` mixing either with another type compiles instead
  of being refused. The corpus query mixes a stored binary field with a
  reference, which is why both members are needed.

## Capabilities

### Modified Capabilities

- `query-repl`: a composite value may carry a unique identifier.

## Impact

One corpus query compiles. The `_B` member is an extension of the codec
over what 1C stores, which the documentation states.
