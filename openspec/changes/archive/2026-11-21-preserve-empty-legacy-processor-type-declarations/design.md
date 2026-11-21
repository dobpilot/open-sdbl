# Design

The retained native compressed Config resource and Designer XML agree on 43 ordered attributes and four empty declarations. Existing declaration() already represents an empty Pattern as an empty vector. Remove the legacy processor fallback check that rejects this valid result. The same native resource contains two nonempty textual comments in identity field 4, matched by Designer XML Comment; preserve these strings rather than requiring an empty string. Non-string comments remain unsupported. Keep identity shape, count, uniqueness and completeness validation unchanged. No change to older class 16 profiles or runtime semantics is required. Empty metadata is not interpreted as Any and does not define assignments or default values.

All packages remain version 0.9.20 pending explicit release approval. Existing tests and expectation files remain unchanged. A separate inherited UUID query corpus failure is reported without rewriting expectations.
