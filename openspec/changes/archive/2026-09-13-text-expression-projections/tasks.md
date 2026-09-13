## 1. Implementation

- [x] 1.1 Add the expression projection cast to the dialect and apply it
  to generated projections in both storage domains.

## 2. Verification and documentation

- [x] 2.1 Goldens for `ЕСТЬNULL`, `ВЫБОР`, an aggregate, a nested query,
  and an unchanged `ВЫРАЗИТЬ(… КАК СТРОКА)`; run the failing queries on
  the live PostgreSQL base.
- [x] 2.2 Run the five CI checks and strict OpenSpec validation.
