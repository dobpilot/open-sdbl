## Context

The rights resource `<guid>.0` decodes to
`{10, {N, {{1, <объект>, K, <члены…>, F}, {0, <право>, <значение>, …}}, …}, {T, {<имя(параметры)>, <тело>}, …}, <флаги>}`.
A rights list opening with `1` carries a count, the pairs, a count of
restriction nodes and the nodes `{<право>, {M, {1, <условие>, L, {<поля>}}, …}}`.
A value `1` grants the right; `-1` records an explicit refusal. The
right identifiers are fixed platform GUIDs; their names follow from the
order the resource writes them in, which is the order of the role editor
(`Read, Insert, Update, Delete, View, …`), confirmed by the single-right
roles of БСП (`ЗапускТонкогоКлиента` and its siblings) and by the
restriction attached to `Read`.

## Decisions

- The record is parsed by shape, not by name, so a resource of
  `ConfigCas` decodes the same way; linking a `ConfigCas` rights record
  to its role needs the extension's container index, which is not read
  yet.
- Rights of every kind are kept in the model, the console applies only
  `Read`: the persistence API to come needs the rest.
- Role names come from the bare-GUID descriptors the library already
  reads; only the list of role identifiers is new.
