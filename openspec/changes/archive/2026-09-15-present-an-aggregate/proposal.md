## Why

`ПРЕДСТАВЛЕНИЕ(МАКСИМУМ(Т.Клиент))` is refused with "aggregate functions
are supported only as projections of a grouped branch", and a corpus query
of a real configuration stops on the same shape.

The platform accepts it. Measured on the probe base against 8.3.27:
`ПРЕДСТАВЛЕНИЕ(МАКСИМУМ(Т.Клиент))` answers the presentation of the
greatest reference ("Завод"), `ПРЕДСТАВЛЕНИЕ(КОЛИЧЕСТВО(*))` answers the
count as text ("13"), and the same under `СГРУППИРОВАТЬ ПО` answers per
group.

The refusal is not a decision about presentations: a projection that
presents an aggregate simply was not counted as an aggregated projection,
so the branch never allowed aggregates in the first place.

## What Changes

- Count a projection that presents an aggregate as an aggregated
  projection, so the branch aggregates and the presentation compiles.

## Capabilities

### Modified Capabilities

- `query-repl`: a presentation may be taken of an aggregate.

## Impact

One corpus query compiles, and four probes that the platform answers stop
being refused. A reference aggregate is presented the deferred way, like
any other reference expression.
