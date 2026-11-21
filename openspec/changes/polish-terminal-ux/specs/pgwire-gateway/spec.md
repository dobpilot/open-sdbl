# PostgreSQL Gateway Delta

## ADDED Requirements

### Requirement: Keep gateway progress and log lines readable together
During startup and metadata refresh, `serve` SHALL use one live stderr
line for measured metadata progress on a suitable terminal. Any log entry
SHALL be written as a permanent line before the latest progress is
redrawn. Static stderr SHALL receive permanent logs and a single terminal
summary per acquisition attempt, with elapsed time and measured counts
when available. The interface SHALL use ordinary terminal scrollback.

#### Scenario: Skipped Config warning interrupts a live update
- **WHEN** a Config frame ends with `14.7 MiB/16.0 MiB` and a warning about
  three unreadable resources arrives while a redraw is pending
- **THEN** the active line is cleared, a permanent `WARN` summary and three
  separate ID and reason lines appear, and the latest progress resumes on
  the following line
- **AND** the terminal never displays the joined text
  `14.7 MiB/16.0 MiBwarning: skipped 3 unreadable`, including the
  `MiBwarning:` boundary, even when decorative effects are disabled

#### Scenario: Redirected gateway diagnostics
- **WHEN** stderr is redirected or live progress is disabled and metadata
  is acquired successfully
- **THEN** stderr contains the warnings and a single metadata completion
  line with resource count, compressed bytes, and elapsed time, with
  neither cursor controls nor intermediate frames
- **AND** stdout stays empty

### Requirement: Keep measured gateway progress aligned
The live `serve` row SHALL show the phase, a measured bar, percent, Config
resource counts, and compressed bytes. A color terminal SHALL show a mint
fill with a bright leading cell. This edge SHALL follow measured progress.
The percentage and counters SHALL use stable columns at a fixed terminal
width while their totals are known.

#### Scenario: Digit transitions keep the bar and counters aligned
- **WHEN** the displayed progress reaches 9%, 10%, and 100% during a
  Config phase with known totals
- **THEN** the bar and counter fields retain their columns and the leading
  cell advances only when the measured fraction increases
- **AND** on successful completion the live row is replaced by one
  summary with the measured counts and elapsed duration

#### Scenario: Terminal becomes too narrow for progress
- **WHEN** a terminal shrinks during a Config read
- **THEN** the bar shortens before the labels are compacted
- **AND** when the row with its measured values no longer fits, repainting
  stops until it fits again and warnings and the final summary remain
  readable in scrollback

### Requirement: Preserve gateway warnings and distinguish failure scope
Gateway operator output SHALL use visible `WARN` and `ERROR` labels and
keep access and plaintext warnings in stderr history. Fatal startup
failures SHALL return a nonzero status. A failed gateway client session
or metadata refresh SHALL be reported locally while the listener continues
to serve other sessions.

#### Scenario: Security warnings without color
- **WHEN** unrestricted access is enabled or incoming passwords are allowed
  without TLS
- **THEN** permanent warnings include `unrestricted access enabled` or
  `no TLS` as applicable and explain the policy before accepting clients,
  even without color or progress
- **AND** an admitted plaintext client is identifiable by a permanent
  warning with its peer and user

#### Scenario: Listener and backing TLS are separate
- **WHEN** `serve` selects `--tls-mode disable` with `--tls-cert` and
  `--tls-key`
- **THEN** only the backing session uses plaintext, and the gateway
  continues to negotiate incoming TLS under the existing listener policy
- **WHEN** a non-loopback listener lacks a certificate and key and has no
  network opt-in
- **THEN** the CLI rejects it before connecting to the backing database,
  regardless of the backing TLS mode

#### Scenario: Session failure with a running listener
- **WHEN** an authenticated client's backing session fails to open
- **THEN** stderr records the peer, failed operation and safe available
  cause in a permanent `ERROR` line
- **AND** the other sessions and listener remain available, and automatic
  disclosure of server error details to clients remains disabled

#### Scenario: Refresh fails while serving
- **WHEN** a SIGHUP metadata refresh fails after reporting progress
- **THEN** stderr finalizes the failed progress and logs the operation and
  available cause, and the gateway continues serving the previous snapshot

## MODIFIED Requirements

### Requirement: Operate the running gateway
The CLI SHALL log connections and failed logins to stderr with peer
address and user name. Passwords and stored values SHALL be logged only
under the explicit sensitive authentication option. On Unix, SIGHUP
SHALL re-acquire metadata and rights and concurrently prebuild the
replacement catalog. Current statements SHALL retain their snapshot.
The CLI SHALL handle interruption during startup, refresh, or serving.

#### Scenario: Failed login logged
- **WHEN** a client fails authentication
- **THEN** standard error names the peer and the user and contains no
  password in normal diagnostics

#### Scenario: Metadata refresh
- **WHEN** the operator sends SIGHUP after updating the configuration
- **THEN** new statements compile against the re-acquired metadata and
  use its complete prebuilt catalog

#### Scenario: Interrupt an active metadata bar
- **WHEN** the operator presses Ctrl+C during a redraw or while a redraw
  and warning are pending during startup or refresh metadata acquisition
- **THEN** the active bar ends, an interrupted summary with elapsed time
  and last measured counts appears on its own line, and shutdown ends
  with a permanent stopped line and status 0
- **AND** any warning is kept whole in history and no progress redraw
  occurs after the interrupted summary

#### Scenario: Interrupt a running listener
- **WHEN** the operator presses Ctrl+C while no metadata bar is active
- **THEN** the gateway shuts down gracefully and stderr ends with a
  permanent stopped line
