# Native separator usage 0.9.13

Measured with 1C 8.3.27.2342 in separate disposable file infobases on 2026-10-09. No demo/buh/unf writes were performed. BSL receipts come directly from the platform, not from SDK execution. Common attribute inventory and catalog scaffolding reuse open-bsl `cfg-src-temp-tables`; the string profile uses `common-attribute-metadata-separate.xml`.

`numeric-attribute.xml` and `numeric-value.xml` replace that profile with matching decimal(10,0), nonnegative types. `numeric-use.bsl` reads both parameters before assignments: use is No, value is 0. Independent false rejects the table; true with initial 0 succeeds.

`shared-attribute.xml` changes only SeparatedDataUse to IndependentlyAndSimultaneously. `shared-use.bsl` observes default false/empty and successful false/true queries. `shared-rows.bsl` uses an empty object module and writes exactly two catalog elements in its own disposable base, with separator values A/B. With true/B the count is 1; false/B and false/C return 2; true/C returns 0. This proves disabled shared access removes filtering rather than applying the empty value.

The type name was verified in installed platform help 8.3.27.2342: `platform-syntax-help:syntax-context:objects/catalog2/catalog370/CommonAttributeSeparatedDataUse/properties/IndependentlyAndSimultaneously8999.html`. An initial unsupported XML spelling failed configuration update and yielded no behavioral receipt; it is not used as evidence.

Replay: use the existing open-bsl native runner in a private copy, change its exact separator-probe script trigger to the chosen BSL path, its common-attribute/value-parameter sources to these fixtures, and set a fresh ONEC_IB. For shared-rows replace ObjectModule with empty-object.bsl. Serialize native creation because ibcmd has a shared working-directory lock.

No assertion is made about unsupported field types, conditional separation, authentication separation, or ordinary unset session parameters.
