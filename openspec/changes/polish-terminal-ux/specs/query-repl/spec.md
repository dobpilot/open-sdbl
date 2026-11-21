# Query REPL Delta

## ADDED Requirements

### Requirement: Represent interactive console initialization with a startup scene
On a supported interactive REPL with terminal stderr, a launch that is
still initializing SHALL display a branch, sphere, or mountain in an
owned region of stderr. Each interactive launch SHALL choose one motif
with probability 1/3 per motif. The choice SHALL persist until startup
ends. Independent launches MAY choose the same motif. The console SHALL
expose no public option for motif selection or animation timing.

#### Scenario: Stable motif during startup
- **WHEN** initialization changes phase or the viewport shrinks and expands
- **THEN** any full scene uses the originally selected motif and the
  latest actual stage and progress
- **AND** a viewport below the artwork's width or height uses a single
  fitting status line with the active stage and any ratio that fits

### Requirement: Scale and shade procedurally generated scenes
The console SHALL generate the selected motif at the current drawable
resolution with a stable per-launch variation. Artwork SHALL scale with
stderr width and available height. Color terminals SHALL use neutral gray and white shading for surfaces
and reserve green for activity and progress accents. Geometry and status
SHALL remain readable in monochrome.

#### Scenario: Widen a drawing in progress
- **WHEN** a terminal grows from 80 to 120 columns with sufficient height
- **THEN** the drawing is sampled at the larger width with the same motif,
  variation, activity phase and measured work at the same instant
- **AND** shrinking back restores the original pose at that instant

#### Scenario: Change color capability
- **WHEN** a fixed scene is rendered in truecolor, indexed color, basic
  ANSI or with NO_COLOR
- **THEN** the body uses neutral tones, the glint uses the single green accent
  in color, and the monochrome frame contains plain glyphs without color sequences

### Requirement: Separate measured console startup work from timed activity
The active stage SHALL be readable throughout. The console SHALL display
measured processed and total counts, with Config resources and compressed
bytes when space permits. An unmeasured stage SHALL show its name without
a fraction. Only initialization events SHALL advance completed work.
Config completion SHALL describe the Config read only.

#### Scenario: A stage has no known amount of work
- **WHEN** the console is connecting, waiting for Config totals, or
  preparing the line editor with no measured totals
- **THEN** the status names the stage and the visible activity can
  continue while the measured reveal stays fixed

#### Scenario: A measured counter stalls
- **WHEN** two renders occur at different times with unchanged Config
  counters
- **THEN** only the impulse, sphere angle, or sheen can change
- **AND** the stage, displayed counts, and completed branch extent,
  filled-band height, or rock depth stay fixed

#### Scenario: Motifs show distinct measured reveals
- **WHEN** measured Config work advances
- **THEN** the branch opens additional paths from its root while
  retaining open paths, the sphere fills dot bands from below while
  keeping meridians and gaps, or the rock shifts upward within its
  region to reveal lower foundation layers
- **AND** a monochrome frame still distinguishes the three silhouettes

#### Scenario: A sphere label passes the edge
- **WHEN** the sphere turns through consecutive views of the same label
  from the front toward a side edge
- **THEN** its strokes compress toward the contour and then hide
  behind it, while the status counter columns remain fixed

#### Scenario: Config is complete and the editor is still initializing
- **WHEN** Config finishes and metadata resolution, commit, or editor
  preparation is still pending
- **THEN** the visible status follows the real active stage and the
  console has no ready prompt or overall completion indication

### Requirement: Hand off the first interactive prompt as soon as it is usable
The console SHALL stop startup rendering and release its terminal region
as soon as the line editor can accept input. The stationary `open-sdbl`
title, working `\help` hint, and ready prompt SHALL appear once with the
first editable line. The handoff SHALL preserve input queued during
initialization in order and exactly once. The startup renderer SHALL
preserve unrelated terminal output and scrollback.

#### Scenario: Fast startup
- **WHEN** the editor is ready before the first animation paint is due
- **THEN** the console displays the ready prompt immediately, with no
  transient frame or wait for a timer

#### Scenario: Paste a full query before the prompt
- **WHEN** a PTY sends a full bracketed paste of `ВЫБРАТЬ 1;` while
  initialization is still pending, then submits the line
- **THEN** the first editable line contains the complete query exactly
  once, the cursor occupies the normal insertion position, and
  submission executes the query once
- **AND** the scene clears before the editor draws the prompt and prior
  terminal history remains readable

#### Scenario: A warning arrives while the scene is drawn
- **WHEN** a diagnostic warning arrives during a paint
- **THEN** it occupies its own permanent stderr line with any resource
  detail lines below it, and the scene resumes beneath the warning
- **AND** the next redraw preserves the complete warning

#### Scenario: Startup fails before the prompt
- **WHEN** connection, acquisition, or editor initialization has a
  fatal failure during startup
- **THEN** the startup region is released, terminal modes and cursor
  are restored, and the operation and available safe cause stay visible
  in stderr
- **AND** no subsequent frame or ready prompt appears and the process
  exits nonzero

#### Scenario: Ctrl+C before the prompt
- **WHEN** the operator presses Ctrl+C while initialization is pending
- **THEN** the scene stops and the console restores the cursor, input,
  and paste modes, leaves a permanent cancellation line, and exits 130

#### Scenario: Static motion and split streams
- **WHEN** an otherwise interactive console has `TERM=dumb`
- **THEN** startup stage lines are plain and follow work events without
  timed repeats
- **WHEN** stdin or stdout is redirected, CI is set, or stderr is redirected
- **THEN** stderr carries permanent diagnostics as plain lines without
  transient startup frames
- **AND** piping stdin or redirecting stdout retains the bounded
  noninteractive input path and omits the startup banner and prompt

#### Scenario: Monochrome console
- **WHEN** nonempty `NO_COLOR` is set on a terminal that supports motion
- **THEN** the scene uses density and strokes without color, and
  measured status and input handoff work as in the colored console

### Requirement: Brand interactive metadata loading without changing the snapshot
The `metadata` command SHALL sweep one mint glint over the word `metadata`
in terminal stderr during acquisition, with a 320 ms cap. Work and exit
SHALL proceed without waiting for the sweep. Warnings and errors SHALL be
written immediately as permanent stderr lines. Successful snapshot stdout
SHALL remain byte-for-byte compatible.

#### Scenario: Pipe the snapshot while watching the progress
- **WHEN** `metadata` stdout is piped and stderr is a capable color terminal
- **THEN** the branding and intermediate frames appear only on stderr

#### Scenario: Identical snapshot with split streams
- **WHEN** the same deterministic input runs with stdout captured
  separately, first with stderr on a PTY and then with stderr in a file
- **THEN** both successful stdout captures match the pre-change reference
  bytes, including whitespace and the final newline
- **AND** terminal controls or startup decoration on stdout would cause
  a mismatch in the unmodified byte streams

#### Scenario: Diagnostic arrives during the sweep
- **WHEN** an acquisition warning or error arrives before the sweep ends
- **THEN** the diagnostic immediately occupies its own permanent line
  and stays visible after any further progress or exit

#### Scenario: Acquisition completes before the sweep
- **WHEN** successful or failed work finishes before the 320 ms cap
- **THEN** the sweep stops immediately and the command completes with its
  normal outcome without waiting for another frame

#### Scenario: Redirect or disable decorative metadata output
- **WHEN** stderr is redirected or static mode is selected
- **THEN** stderr omits the title and frames while retaining errors and warnings

#### Scenario: Metadata progress without color
- **WHEN** only `NO_COLOR` is nonempty on a capable terminal
- **THEN** the live phase and counters remain readable without the title sweep

### Requirement: Preserve safe PostgreSQL failure information
Local CLI diagnostics for PostgreSQL SHALL retain the operation, SQLSTATE
when the driver supplies it, and a safe description of the cause. Trusted
layout SHALL be rendered as real lines and untrusted fields SHALL be
escaped. Diagnostics SHALL omit passwords, connection strings, raw SQL,
and unchecked server details.

#### Scenario: PostgreSQL cannot read the metadata source
- **WHEN** the DBNames read fails with server SQLSTATE `42P01` for the
  expected `params` relation
- **THEN** stderr names the PostgreSQL DBNames read, shows `42P01` and
  the missing relation, and tells the operator to check that the target
  database is a 1C information base
- **AND** a failed standalone metadata run leaves stdout empty and exits 2

#### Scenario: Recoverable console query error
- **WHEN** PostgreSQL returns a recoverable query error
- **THEN** the console reports the failed operation and any safe
  available cause and SQLSTATE, reports elapsed phase time, rolls back
  the statement transaction, and accepts the next query

#### Scenario: No server error record
- **WHEN** the driver reports a connection or I/O failure without a server
  SQLSTATE
- **THEN** the CLI prints the operation and available safe cause without
  inventing a SQLSTATE or a remedy

## MODIFIED Requirements

### Requirement: Provide an interactive PostgreSQL REPL
The `open-sdbl-cli` package SHALL provide `open-sdbl console postgres` using
the existing connection and authentication options, with `repl` retained as a
compatibility alias. It SHALL load metadata at startup, accept
semicolon-terminated multiline UTF-8 queries, provide current-session command
history and completion on interactive terminals, and provide syntax
highlighting when styling is enabled. It SHALL protect interactive Linux
terminal editing with `IUTF8`, recover from a byte-invalid input line, and
execute every generated statement in a verified read-only
`READ COMMITTED` transaction.

#### Scenario: Interactive command hint
- **WHEN** the console enters regular editing on a capable interactive
  terminal and the footer fits
- **THEN** a compact footer for `\dt`, `\di`, `\d`, `\refresh`, `\help`, and
  `\q` remains on the final terminal row while normal output scrolls above it

#### Scenario: Syntax highlighting
- **WHEN** the user edits a lexically complete SDBL query with colors enabled
- **THEN** keywords, literals, comments, parameters, and known metadata names
  are displayed with distinct ANSI styles without changing the query text

#### Scenario: Metadata-aware completion
- **WHEN** the user presses Tab after a partial command, keyword, object, or
  field name
- **THEN** the line editor offers case-insensitive candidates derived from
  console commands and the current resolved metadata snapshot

#### Scenario: Recall query history
- **WHEN** the user presses Up or Down in an interactive console
- **THEN** the line editor navigates queries and commands entered earlier in
  the current console session

#### Scenario: Query execution
- **WHEN** the user enters a supported 1C query terminated by `;`
- **THEN** the CLI displays SDBL-to-SQL generation time, exact generated SQL,
  PostgreSQL execution time, column labels, rows, and row count and then
  prompts for the next command

#### Scenario: Recoverable error
- **WHEN** compilation or PostgreSQL execution fails
- **THEN** the console prints the error and elapsed phase time, rolls back the
  statement transaction, and remains available

#### Scenario: Recoverable byte-invalid input
- **WHEN** one input line is not valid UTF-8
- **THEN** the console discards the affected statement, reports the input
  error, and accepts the next command without closing the database connection

#### Scenario: UTF-8 terminal editing
- **WHEN** the console runs on an interactive Linux terminal with `IUTF8`
  disabled
- **THEN** it enables `IUTF8` while reading commands and restores the previous
  terminal attributes before exit

#### Scenario: Session lifecycle
- **WHEN** input reaches EOF or the user enters `\q`
- **THEN** the CLI restores terminal state, closes the PostgreSQL connection,
  and exits successfully

### Requirement: Report metadata-loading progress without contaminating output
For the standalone `metadata` command on PostgreSQL and MSSQL, the CLI
SHALL display one rate-limited stderr progress line on a capable terminal
with the phase, Config percent, processed/total resources, and compressed
bytes. Counters SHALL be measured and unknown totals SHALL be indicated
explicitly. Successful interactive completion SHALL include elapsed time.
The interactive console SHALL use its startup scene or single-line stage
fallback during acquisition. Both commands SHALL retain permanent
diagnostics when stderr is redirected.

#### Scenario: Interactive metadata loading
- **WHEN** `metadata` acquires a snapshot with stderr attached to a
  capable terminal
- **THEN** the user sees phase changes and Config transfer reach 100%
  when the measured counters reach the known totals

#### Scenario: Redirected metadata snapshot
- **WHEN** `open-sdbl metadata postgres` stdout is redirected or piped
- **THEN** stdout contains only the existing tabular snapshot records

#### Scenario: Noninteractive diagnostics
- **WHEN** standard error is not a terminal
- **THEN** progress rendering is suppressed while ordinary errors and
  warnings remain available on standard error

#### Scenario: Zero and unknown totals
- **WHEN** Config totals in the standalone `metadata` line are still unknown
  or known to be zero
- **THEN** unknown totals and percent are shown as unknown, and a known
  empty transfer displays zero counts and reaches completion without
  fabricating work

### Requirement: Secure PostgreSQL transport by default
The CLI SHALL default PostgreSQL to encrypted transport with certificate
chain and hostname verification, SHALL honor `PGSSLMODE` in the absence
of an explicit mode, and SHALL require an explicit plaintext opt-in.
Private trust roots SHALL apply only in a verifying mode.

#### Scenario: Default connection
- **WHEN** the user connects without transport flags or `PGSSLMODE`
- **THEN** the connection uses TLS with full verification or fails with
  a diagnostic

#### Scenario: Explicit plaintext opt-in
- **WHEN** the user supplies legacy `--sslmode disable` or
  `PGSSLMODE=disable` without `--insecure-plaintext`
- **THEN** the CLI refuses to connect and names the required legacy opt-in
  and the explicit canonical equivalent
- **WHEN** the user supplies `--tls-mode disable` or the valid legacy
  combination
- **THEN** the CLI uses plaintext and warns before connecting

#### Scenario: Private PostgreSQL CA
- **WHEN** a private CA file is supplied in `verify-ca` or
  `verify-full` mode
- **THEN** PostgreSQL TLS validates the server chain against that CA
  instead of the system roots and validates the hostname in
  `verify-full` mode

### Requirement: Warn when certificate verification is disabled
The CLI SHALL warn on stderr before using an encrypted backing connection
without certificate and hostname verification. The CLI SHALL offer
`--trust-ca-file` with a verifying mode for private or self-signed
deployments.

#### Scenario: Trust flag warning
- **WHEN** the user passes `--trust-server-certificate` for MSSQL or
  selects `require` for either provider
- **THEN** a warning explains that encryption is enabled and the peer
  certificate and hostname will be unverified

### Requirement: Preserve trusted CLI diagnostic layout
Argument diagnostics SHALL render the built-in short usage layout with
real line breaks. Diagnostics SHALL escape dynamic text before it reaches
the terminal. The missing PostgreSQL legacy plaintext opt-in SHALL retain
its stable plain-text code, summary, cause, and remediation fields.

#### Scenario: Missing plaintext opt-in
- **WHEN** legacy PostgreSQL plaintext mode is requested without its
  explicit insecure opt-in flag
- **THEN** stderr includes `OPEN_SDBL_CLI_PG_PLAINTEXT_OPT_IN_REQUIRED`,
  explains the cause, and names the legacy opt-in, canonical alternative,
  and verified default without ANSI styling or a full manual

#### Scenario: External error text
- **WHEN** an argument or runtime error field contains terminal control
  characters or bidirectional formatting characters
- **THEN** they appear in escaped textual form and the built-in line
  breaks remain readable

### Requirement: Skip unreadable Config resources
A Config resource that fails UTF-8 or brace-serialization decoding SHALL
be skipped while metadata is acquired from the rest. The CLI SHALL write
a permanent stderr summary and one readable line per skipped resource
with its safe ID and cause. Suppressing progress SHALL preserve this
warning.

#### Scenario: Binary predefined resource
- **WHEN** a `.7` resource starts with bytes that are not UTF-8
- **THEN** the CLI names it and the decoding reason in a warning and
  acquires metadata from the remaining resources

#### Scenario: Multiple skips with progress hidden
- **WHEN** four unreadable Config resources are decoded with stderr
  redirected
- **THEN** stderr reports four skipped resources with four ID and cause
  lines, without terminal control bytes or repeat frames
