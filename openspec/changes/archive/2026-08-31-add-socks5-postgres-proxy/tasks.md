## 1. CLI contract

- [x] 1.1 Add and validate `--socks5-proxy HOST:PORT` in the shared PostgreSQL
  option parser and document it in command help.
- [x] 1.2 Add CLI regression coverage for option discovery and invalid proxy
  endpoints.

## 2. SOCKS5 transport

- [x] 2.1 Negotiate an unauthenticated, timeout-bounded SOCKS5 CONNECT tunnel
  for IPv4, IPv6, and proxy-resolved domain targets.
- [x] 2.2 Run `tokio-postgres` over the negotiated stream for metadata and
  console sessions while preserving the direct path.
- [x] 2.3 Add deterministic local proxy tests for target encoding and failure
  diagnostics.

## 3. Verification

- [x] 3.1 Run workspace formatting, Clippy with warnings denied, tests, and
  rustdoc with warnings denied.
- [x] 3.2 Run strict OpenSpec validation for the completed change.
