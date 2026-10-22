# Decode stored value content as text

## Why
A consumer already has its native-value decoder and needs the UTF-8 serialized content of an inline ValueStorage envelope. Reconstructing text from the SDK Value tree or duplicating framing/inflation in the consumer would introduce a second codec.

## What Changes
Add a pure decode_stored_value_text(content, limit) API returning Cow<str>. Share existing framing and bounded raw-DEFLATE extraction with decode_stored_value. Enforce the decoded-byte bound for uncompressed content as well as compressed content. Preserve successful existing tree cases.

## Impact
Additive core API, no I/O/dependencies or version change in this isolated draft. Release version remains pending.
