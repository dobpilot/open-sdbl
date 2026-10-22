## ADDED Requirements

### Requirement: Use patched PostgreSQL TLS dependencies
The database crate SHALL require rustls 0.23.46 or newer and webpki-roots
1.0.10 or newer in their compatible release series. The release dependency
graph SHALL resolve rustls-webpki to at least 0.103.16 in its compatible series.
The existing CA selection and hostname verification settings SHALL remain
unchanged. Upstream rejection of distrusted certificate paths SHALL apply.

#### Scenario: Release dependency audit
- **WHEN** the release dependency graph is audited
- **THEN** its resolved TLS versions are outside the affected ranges of RUSTSEC-2026-0346, RUSTSEC-2026-0347, RUSTSEC-2026-0348, RUSTSEC-2026-0349, RUSTSEC-2026-0381 and RUSTSEC-2026-0382

#### Scenario: Distrusted certificate path
- **WHEN** a server certificate path is rejected by the patched verifier or relies solely on a removed bundled trust anchor
- **THEN** PostgreSQL TLS connection establishment fails rather than bypassing validation
