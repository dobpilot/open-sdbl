# Share bounded V8 container entry decoding

## Why
Native common settings store the internal value in a V8 container. The existing open-bsl configuration loader already decodes this container; consumers need one shared decoder rather than another implementation.

## What Changes
Lift the existing decoder into the dependency-free SDK metadata API, with an explicit per-document byte bound and malformed-chain errors. Reuse it from the configuration loader when integrating this SDK draft.

## Impact
Additive pure Rust metadata APIs; no database writes, BSL values or new dependencies.

Expose decoded common-setting text with an explicit distinction between an internal value and multipart ValueStorage contents, reusing the existing stored-value decoder.
