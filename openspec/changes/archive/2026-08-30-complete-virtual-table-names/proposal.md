# Change: Complete qualified virtual-table names

## Why

The console keeps dots inside the active completion token, but its candidate
catalog contains only standalone virtual-table keywords. Consequently Tab
cannot complete a qualified source such as
`РегистрНакопления.Остатки.Ос`.

## What changes

- Generate qualified Russian and English virtual-table candidates according to
  each resolved register kind.
- Include the required empty argument list in replacements so a completed
  no-argument source is immediately valid for the current parser.
- Rebuild those candidates on the existing metadata refresh path.
