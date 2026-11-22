# Decode extension purpose with compatibility 8.3.10

## Why

Session startup in UNF reaches an active extension whose configuration header uses an older form. The native exporter exposes its purpose, but the SDK decoder currently returns `None`.

## What changes

- Decode purpose in the native-measured legacy header with extension compatibility 8.3.10.
- Also decode the same compatibility mode in the header emitted by 1C 8.3.27 when it saves a new CFE.
- Keep the existing unknown-purpose behavior for unsupported or damaged inputs.
