# Design

The version 1 header uses identity `{0,0,<id>}` and a tail of the form `<belonging>,<n>,(<guid>,<tag>)×n`. The first field gives the belonging. Preserve the existing count and pair checks before interpreting that field. Version 1 carries no explicit target field, so both belonging values leave `extends` empty.

The controlled platform comparison is documented in `tests/fixtures/extension/own-v1/README.md`. The version 3 decoder and extension acquisition continue to use their existing paths.
