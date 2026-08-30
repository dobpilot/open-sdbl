# Design: Directional information-register slices

The private `SliceLast` AST becomes a generic slice node carrying
`SliceKind::{First, Last}`. Both kinds retain the same authoritative table,
Period, Config-dimension, data-separator, condition, alias, and JOIN handling.

PostgreSQL generation continues to use a derived relation with `DENSE_RANK()`
so all records tied at the selected Period survive. SliceFirst orders Period
ascending and treats its optional period as an inclusive lower bound (`>=`);
SliceLast orders descending and retains its inclusive upper bound (`<=`). The
virtual condition is evaluated in the ranked input and a normal WHERE remains
outside.

The optional `_InfoRgSFN` totals table is not required. This matches the test
information base, where no slice totals table exists, and keeps historical
period boundaries available through the authoritative `_InfoRgN` main table.

