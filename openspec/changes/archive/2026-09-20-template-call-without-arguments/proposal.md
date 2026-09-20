## Why

A role of «1С:Документооборот» restricts a catalog with the text
`#ЧтениеШаблоновПроцессов` — a call of a template that takes no
argument, written without the parenthesis. The library reads a call only
as `#Имя(…)`, so the name survives expansion and the compiler stops at
the `#` it never expected.

## What Changes

- A `#Имя` naming a template of the role SHALL be a call of it whether
  or not a parenthesis follows, with no arguments when it does not.

## Capabilities

### Modified Capabilities

- `access-rights`: a template call without arguments.

## Impact

`src/access.rs` (`expand_templates`, `scan_references`).
