# CLI Configuration Delta

## ADDED Requirements

### Requirement: Render explicit command help
The CLI SHALL print help to stdout and exit with status 0 for an explicit
`--help` or `-h` in a recognized command scope. The first paint SHALL include
the `open-sdbl` title, usage, applicable commands or options, and an example.
Usage SHALL be prominent immediately after the title. Colored help SHALL
use one inline title gradient in ordinary scrollback with no timed frames
or repaint.

#### Scenario: Top-level and scoped help
- **WHEN** the user runs `open-sdbl --help`, `open-sdbl serve --help`, or
  `open-sdbl console postgres --help`, or `open-sdbl metadata --help`
- **THEN** each command exits 0 with its applicable help in stdout and empty
  stderr
- **AND** root help lists `lex`, `metadata`, `console`, and `serve`, and
  scoped help groups options by task, shows where to find advanced and
  compatibility options, and lists shared flags once

#### Scenario: Help bypasses startup
- **WHEN** the user runs
  `open-sdbl serve postgres --listen 0.0.0.0:5433 --help` or
  `open-sdbl serve --config missing.toml --help`
- **THEN** the matching help is printed once with exit status 0 before
  listener validation, configuration reads, credentials resolution, or
  database access

#### Scenario: Help fits ordinary scrollback
- **WHEN** the user opens help in a narrow PTY
- **THEN** usage, commands or options, and the example remain complete,
  and the help can be copied once as ordinary text without cursor motion

#### Scenario: Pager and file output
- **WHEN** the user runs `open-sdbl --help | less` or redirects stdout to a
  file
- **THEN** the consumer receives a single complete page with no terminal
  sequences

### Requirement: Report command failures with scoped diagnostics
Argument errors SHALL emit a specific problem and short applicable usage on
stderr, with a scoped help command where useful, and exit 2. Execution
failures SHALL emit the operation, the safe available cause, and a next
action when one is known, and SHALL exit nonzero. The full manual SHALL
be reserved for the help path. Errors SHALL be permanent stderr lines.

#### Scenario: Unknown option and missing value are distinct
- **WHEN** the user runs `open-sdbl serve postgres --wat` or
  `open-sdbl serve postgres --listen`
- **THEN** stderr identifies the unknown option or missing value
  respectively and shows relevant `serve` usage, stdout is empty, and exit
  status is 2

#### Scenario: Missing metadata options
- **WHEN** the user runs `open-sdbl metadata postgres --host db`
- **THEN** stderr names the missing options and the scoped help command,
  includes short metadata usage, and leaves stdout empty with status 2

#### Scenario: Invalid configuration and failed file access
- **WHEN** a connection command reads a malformed TOML file or cannot open
  its configured file
- **THEN** the former is reported as an argument error with the offending
  field or parse location when available, and the latter as an execution
  failure naming the file and the I/O cause
- **AND** neither diagnostic copies raw TOML source or secret values to
  stderr

#### Scenario: Metadata or serve cannot read a startup file
- **WHEN** `metadata` cannot open its configuration file or `serve`
  cannot open its listener certificate file
- **THEN** stderr identifies the failed operation, path, and I/O cause,
  stdout is empty, and exit status is 2

#### Scenario: Listener certificate file has no certificate
- **WHEN** `serve` reads an empty listener certificate file
- **THEN** stderr identifies the TLS setup operation, path, and missing
  certificate, stdout is empty, and exit status is 1

#### Scenario: PostgreSQL rejects the backing TLS certificate
- **WHEN** the driver rejects an untrusted certificate issuer or a hostname
  mismatch while establishing a PostgreSQL connection
- **THEN** stderr identifies the connection operation and the confirmed
  certificate cause, and suggests checking `--trust-ca-file` or `--host`
  respectively
- **AND** stdout is empty, exit status is 2, and diagnostics omit secrets

#### Scenario: Execution fails after progress
- **WHEN** the gateway has reported progress and then cannot complete
  startup
- **THEN** stderr contains a permanent error on a new line, stdout stays
  empty, and the process returns its nonzero execution status

### Requirement: Render output according to its destination
The CLI SHALL determine terminal capabilities independently for stdout and
stderr. Redirected streams, CI, and unsupported terminals including
`TERM=dumb` SHALL receive static output without styling or cursor
controls. A nonempty `NO_COLOR` SHALL suppress color and title sweeps.
Labels and diagnostic meaning SHALL survive styling changes. The
interactive console startup scene MAY retain monochrome activity.

#### Scenario: Independently redirected streams
- **WHEN** stdout is a pipe or file and stderr is a supported terminal
- **THEN** stdout contains static text or existing data records, and
  stderr can render live progress for standalone `metadata` or `serve`.
  A batch console keeps plain diagnostics
- **WHEN** stderr is a pipe or file and stdout is a supported terminal
- **THEN** stderr contains static diagnostics and applicable final status
  lines without frames, and stdout can style explicit help

#### Scenario: Environment overrides on a PTY
- **WHEN** the process runs in CI or with `TERM=dumb` on a PTY
- **THEN** both output streams are static without terminal controls

#### Scenario: Monochrome progress is readable
- **WHEN** nonempty `NO_COLOR` is set on a capable terminal with movement
  otherwise available
- **THEN** the standalone `metadata` title sweep and all color are absent,
  while labels, measured progress, and the REPL scene's strokes and
  silhouettes remain readable

### Requirement: Use terminal-friendly presentation colors
On a capable color terminal the CLI SHALL use a mint accent for title
and prompt text and filled progress cells, amber for `WARN`, and red for
`ERROR`. Message bodies, paths, and IDs SHALL use the terminal's default
foreground on its default background. The CLI SHALL use its fixed palette
and timing without user tuning flags.

#### Scenario: Dark, light, and basic color terminals
- **WHEN** a dark or light truecolor terminal or a basic ANSI terminal
  displays the same message
- **THEN** accents remain visible and readable, the message can be copied
  as ordinary text, and its meaning is identical in monochrome

### Requirement: Select backing database TLS consistently
`console`, `repl`, `metadata`, and `serve` SHALL accept `--tls-mode MODE`
and the TOML key `tls_mode`. Both providers SHALL accept `disable`,
`require`, and `verify-full`. PostgreSQL SHALL also accept `verify-ca`.
The mode SHALL govern the backing database connection independently of
listener TLS.

#### Scenario: One explicit plaintext spelling
- **WHEN** a command selects PostgreSQL or MSSQL with `--tls-mode disable`
  or `tls_mode = "disable"`
- **THEN** it warns on stderr before connecting and opens a backing
  connection without TLS only if the server permits it
- **AND** a server that requires TLS causes a diagnostic and connection
  failure before the client sends a password and without a mode switch

#### Scenario: Preserve verified defaults
- **WHEN** either provider is selected with no explicit TLS or trust
  settings and with `PGSSLMODE` unset
- **THEN** the backing connection uses encrypted transport with
  certificate-chain and hostname checks

#### Scenario: Encrypted modes
- **WHEN** either provider is given `--tls-mode require` or
  `--tls-mode verify-full`
- **THEN** the former requires encryption without certificate or hostname
  verification and emits a warning, and the latter requires encryption with
  chain and hostname verification
- **WHEN** the user selects `verify-ca`
- **THEN** PostgreSQL requires encryption with chain verification, and
  MSSQL reports an unsupported mode before connecting

#### Scenario: Conflicting choices
- **WHEN** canonical and legacy TLS selection flags or their active TOML
  counterparts are combined, differing canonical modes repeat, or
  `--trust-ca-file` is combined with `disable` or `require`
- **THEN** the CLI identifies the conflict in an argument error before
  connecting
- **WHEN** the same canonical mode is repeated
- **THEN** it is treated as one selection with at most one transport warning

#### Scenario: Legacy settings and environment
- **WHEN** PostgreSQL uses the legacy `--sslmode` and
  `--insecure-plaintext` or MSSQL uses `--trust-server-certificate`,
  including equivalent legacy TOML fields
- **THEN** accepted invocations keep their provider, mode, and exit behavior
  without a separate migration warning
- **WHEN** PostgreSQL receives an explicit canonical or legacy mode as well
  as `PGSSLMODE`
- **THEN** the explicit mode wins
- **AND** neither a default nor `PGSSLMODE=disable` alone enables plaintext

#### Scenario: Reuse a gateway file
- **WHEN** console or metadata reads a valid gateway TOML file containing
  `tls_mode`, `tls_cert`, and `tls_key`
- **THEN** it applies the backing mode, validates the listener fields'
  types, and does not use the listener credentials
