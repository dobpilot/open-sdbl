# Design: Safe movement-table aggregation

Config collection GUIDs classify accumulation-register fields without using
index or physical-name heuristics: `b64d9a43-1642-11d6-a3c7-0050bae0a776`
identifies dimensions, `b64d9a41-1642-11d6-a3c7-0050bae0a776` resources, and
`b64d9a42-1642-11d6-a3c7-0050bae0a776` attributes.

The compiler builds a derived relation from the authoritative `_AccumRgN` main
table. It filters inactive rows, groups every physical member of Config
dimensions and data separators, and aggregates resources under their existing
physical column names. A balance register is identified by its live RecordKind
field; receipt (`0`) contributes positively and expense contributes negatively.
A turnover-only register has no RecordKind and sums stored resources directly.

`Balance([period][, condition])` uses an exclusive point boundary (`Period <
period`), omits groups whose every resource balance is zero, and is rejected for
a turnover-only register. `Turnovers([begin][, end][, periodicity][, condition])`
uses a half-open `[begin, end)` interval. The initial bounded implementation
requires the periodicity slot to be omitted; supporting grouped Period output is
a later change. All date arguments are scalar literals and virtual conditions
use direct dimension/separator fields only. An outer WHERE remains after
aggregation.

The implementation deliberately reads movement rows rather than depending on
optional `_AccumRgT`/`_AccumRgTn` totals. That gives one correct path for current
and historical periods and for totals-disabled databases. Using totals as an
equivalent query-plan optimization is deferred.

