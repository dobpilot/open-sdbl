# Design

Measure using native ibcmd 8.3.27.2342 on disposable PostgreSQL sdbl_measure_095b. Original databases remain untouched. Each update is paired with native extension info and a read-only raw _ExtensionsInfo record.

A native update of unsafe-action protection additionally generated the a1 variant; a preceding synthetic replay in the disposable base was restored and is separately identified in the evidence. The known record prefix is marker 43c29a14, a 20-byte generation root key, standalone a1/a2 (native unsafe-action protection false/true) and a counted eight-byte update counter. The next standalone Boolean records safe mode, followed by the profile field. Decode that measured prefix and require a complete structural walk before answering mode/profile/hash. Unknown/truncated records remain None. Counted profile content must not become the extension version.

Native HashSum matches Base64 of the generation root key. Expose hash_sum independently of content_hash rather than changing the latter's representation. Reuse one ContentKey Base64 encoder for production and fixture builders.

Native InfoBase scope is raw code 1. The native platform refuses DataSeparation without separators in this test configuration: no other numeric code is mapped. Preserve the raw code and answer a typed InfoBase only for 1.
