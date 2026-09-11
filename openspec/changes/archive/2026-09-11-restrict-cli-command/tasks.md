## 1. Session parameters

- [x] 1.1 Parse `\session`, `\session <Имя> [=] <литерал>`, and
  `\session clear`; store values in a second `ParameterStore` and expose
  them as `SessionParameters`.

## 2. Restrictions

- [x] 2.1 Add `RestrictionStore` with `\restrict <имя> <условие>`,
  `\restrict`, and `\restrict clear`; resolve the target at entry time.
- [x] 2.2 Pass the requested restrictions and the session parameters into
  `CompileOptions` for every query.

## 3. Console integration and documentation

- [x] 3.1 Dispatch the commands in the REPL loop, extend `\help`, command
  completion, and `&` completion.
- [x] 3.2 Unit tests for command parsing, storing, listing, clearing, and
  request filtering; update README and `docs/query-language-support.md`;
  run the five CI checks and strict OpenSpec validation.
