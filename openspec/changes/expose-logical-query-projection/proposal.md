# Expose logical query result projection

## Why
Physical output columns spread a composite selected value over payload members and a discriminator. Consumers cannot recover SELECT occurrences or full aliases from physical labels/origins, especially after CASE flattening and UNION normalization.

## What Changes
Add a structured complete-or-unavailable logical projection descriptor to CompiledQuery. Preserve existing SQL, physical columns, service columns, nested results, compilation acceptance and dependencies. Keep SDK version 0.9.15 in this isolated draft; no main/release changes.

## Impact
query-compilation; core output metadata only. Row decoding remains the consumer responsibility. No new query parser or storage/value codec.
