## Why
The release CI audit reports RUSTSEC-2026-0346, 0347, 0348 and 0349 for
rustls 0.23.45, RUSTSEC-2026-0382 for rustls-webpki 0.103.15 and
RUSTSEC-2026-0381 for webpki-roots 1.0.9. Release dependencies must use
the upstream fixes, including rejection of distrusted certificate paths.

## What Changes
- Raise the database crate's rustls and webpki-roots minimum versions to
  0.23.46 and 1.0.10, respectively.
- Resolve rustls-webpki to patched 0.103.16 or newer within the compatible series.
- Preserve configured CA handling, hostname verification and provider APIs.

## Impact
PostgreSQL TLS dependencies and the release lockfile. Certificate paths
rejected by the patched upstream verifier or removed trust anchors are no
longer accepted. No new dependency or I/O is added to the root library.
