# Design: Shared bounded aggregate AST

The private AST stores `AggregateKind::{Count, Sum, Min, Max}`, the original
function token, optional DISTINCT, and either wildcard or one field argument.
Only COUNT accepts wildcard and DISTINCT in this change. SUM, MIN, and MAX
require one resolved logical field. Single-member values compile directly; a
pure compound reference uses its RRef member consistently with COUNT.

PostgreSQL aggregate results are cast to text for the existing CLI transport.
The existing pre-SQL checks for mixed aggregate/non-aggregate projections and
transposed FULL JOIN branches apply uniformly to every aggregate kind.
