# Design: Derived latest-period relation

Config property collection GUIDs are decoded into a typed field purpose. For
information registers, `13134203-f60b-11d5-a3c7-0050bae0a776` identifies
dimensions, `13134202-f60b-11d5-a3c7-0050bae0a776` resources, and
`a2207540-1400-11d6-a3c7-0050bae0a776` attributes. The compiler does not infer
dimensions from index names.

The source compiles to a derived relation over the authoritative main table.
`DENSE_RANK() OVER (PARTITION BY <all physical dimension and separator members>
ORDER BY Period DESC)` preserves every record tied at the greatest eligible
period. An omitted dimension list produces one global partition. Optional
period and condition predicates are placed inside the ranked input; the normal
query WHERE remains outside.

The bounded first implementation accepts a scalar literal for the period and
the existing direct-field expression subset for the virtual condition. Query
parameters and reference-property dereferences in that condition remain
unsupported. Derived sources participate in ordinary JOINs; existing FULL JOIN
transposition treats them as relations without changing slice semantics.

The optional physical `_InfoRgSLN` totals table is not required. This keeps the
result correct for information bases, including the conformance `test` base,
where SliceLast totals are disabled.

