## 1. Native rendering

- [x] 1.1 Render `ПОЛНОЕ [ВНЕШНЕЕ] СОЕДИНЕНИЕ` as `FULL JOIN` in source
  order and delete the `UNION ALL` transposition together with its
  anti-match marker.
- [x] 1.2 Drop the refusals the transposition needed: a full join in a
  chain, aggregates over it, and `СГРУППИРОВАТЬ` with it.

## 2. Conditions

- [x] 2.1 Allow a dereference in a full join condition by resolving the
  reference join inside the side that owns it.
- [x] 2.2 Keep refusing a condition that is not an equality chain, since
  the server plans a full join only on merge- or hash-joinable
  conditions.

## 3. Minimum server

- [x] 3.1 Record PostgreSQL 13 as the minimum target in `CLAUDE.md`,
  `README.md`, and the specs, replacing "portable to 9.0".

## 4. Verification

- [x] 4.1 Re-record the goldens and the corpus, and replace the tests that
  assert the transposition.
- [x] 4.2 Compare the probe answers with the platform for a full join in a
  chain, two full joins, grouping, aggregation, a dereferencing condition
  and a `ГДЕ` over the join.
- [x] 4.3 Run formatting, Clippy with warnings denied, workspace tests,
  rustdoc with warnings denied, the bounded fuzz checks, and strict
  OpenSpec validation.
