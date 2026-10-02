# Design

The existing SDK request explicitly permits either a full fingerprint or an honest narrow contract. Choose the latter to preserve SQL and cost. CONFIG_FINGERPRINT hashes rows selected by the same Config predicate as metadata loading: bare GUID or .1c/.9/.7 suffix. It excludes other resources and ConfigCAS/_ExtensionsInfo/_ExtensionsRestruct. Equal results do not prove freshness of complete or selected configuration acquisition or of a resolved snapshot. READ COMMITTED remains unchanged.

Document the boundary at both query constants, AcquiredConfiguration and README. Retain archived changes unchanged. Add a regression proving both provider fingerprints use the exact filtered metadata predicate, whereas whole-acquisition queries do not. Verify selected/ineligible resource names via the existing public predicate; no second decoder or SQL path.
