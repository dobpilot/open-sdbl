## Why

The restriction templates are decoded as text and expanded by
substitution: nothing reads their structure. An operator cannot see what
a template is made of without reading tens of thousands of characters,
and a template that is malformed is only reported when a query happens to
expand it, with no position in the text.

## What Changes

- The library SHALL parse a restriction text or a template body into its
  nodes — text, conditions with their branches, template calls with their
  arguments, numbered parameters and names — each with its byte offset,
  and SHALL report a malformed text with the offset.
- The call scanner SHALL be one: expansion and the parser read a call the
  same way.
- `\template <роль>` SHALL say whether each template parses, and
  `\template <роль> <имя>` SHALL print what the body is made of beside it.

## Capabilities

### Modified Capabilities

- `access-rights`: the nodes of a restriction text.
- `query-repl`: the console reports the structure of a template.

## Impact

`src/access.rs` (`TemplateNode`, `TemplateBranch`, `parse_template`,
`RestrictionError::SyntaxAt`), `crates/open-sdbl-cli/src/access.rs`.
