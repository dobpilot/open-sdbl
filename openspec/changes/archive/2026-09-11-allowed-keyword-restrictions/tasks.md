## 1. Syntax

- [x] 1.1 Add `Keyword::Allowed` (`РАЗРЕШЕННЫЕ`/`ALLOWED`) to the lexer table
  and the exhaustive keyword tests.
- [x] 1.2 Parse the keyword after `ВЫБРАТЬ`, hoist it to the statement, and
  reject it in later union branches and nested queries.

## 2. Public API and parameters

- [x] 2.1 Add `SessionParameters`, `AccessRestriction`, `RestrictionTarget`,
  `RestrictionRequest`, the `CompileOptions` builders, and
  `Prepared::restriction_request`; thread `CompileOptions` through the
  codegen entry points.
- [x] 2.2 Resolve `&Имя` against query parameters first, then session
  parameters; exempt session parameters from the unused check.

## 3. SQL generation

- [x] 3.1 Record restriction targets on the statement catalog, match
  supplied restrictions, and report unused or duplicate restrictions.
- [x] 3.2 Compile restriction text through `Parser::parse_condition` with a
  single-scope context, session-only parameters, and the keyword flag
  cleared; wrap plain sources in the `__restricted` derived table.
- [x] 3.3 Conjoin restrictions into slice, balance, and turnover predicates.
- [x] 3.4 Add `QueryDiagnosticKind::Restriction` and wrap inner failures
  with the target label.

## 4. Verification and documentation

- [x] 4.1 Goldens on both dialects: plain source, joined source, tabular
  section, extension `UNION ALL` source, dereference and nested `В` inside
  a restriction, slice and balance conjunction, `ПОМЕСТИТЬ` under the
  keyword, statements without the keyword untouched, request contents;
  diagnostics for keyword placement, unused and duplicate restrictions,
  invalid restriction text, missing session parameter.
- [x] 4.2 Update README and `docs/query-language-support.md`; run
  formatting, Clippy, workspace tests, rustdoc, and strict OpenSpec
  validation.
