## Why

`Задача.ЗадачаИсполнителя.Предметы.Предмет = Файлы.Ссылка` compares a
column of a tabular section that holds a composite reference. The
`EXISTS` this project generates for a section predicate takes only a
single-column field, so the query — the last real gap of the corpus —
still stops.

No new semantics are involved: the existence meaning of a section
predicate was measured on 8.3.27 (an owner answers once however many of
its rows match), and comparing a composite reference with a fixed one was
measured earlier — the pair `RTRef ‖ RRRef` is compared with the payload
of the other side. This change only joins the two.

## What Changes

- Compare a composite column of a tabular section inside the `EXISTS` by
  its `RTRef ‖ RRRef` payload, widening the other side to a payload the
  same way an ordinary comparison does.

## Capabilities

### Modified Capabilities

- `query-repl`: a section predicate may compare a composite reference.

## Impact

The last corpus query that stops on a real gap compiles.
