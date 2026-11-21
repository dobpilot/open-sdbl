# Tasks

## 1. Diagnostics and transport compatibility

- [x] 1.1 Resolve root, command, and provider help ahead of config and startup.
  Assert status 0, stdout-only output, and one complete help page for
  `--help`, `-h`, an invalid serve listener followed by help, and a missing
  config followed by help. Keep no-argument help.
- [x] 1.2 Accept diagnostics independently of styling. Extend
  `crates/open-sdbl-cli/tests/metadata_cli.rs` and the serve CLI tests.
  Check `metadata postgres --host db`, `serve postgres --wat`, missing
  values, malformed TOML, and the legacy plaintext opt-in error
  for specific stderr, short usage, scoped help, empty stdout, and status
  2. Check missing config and certificate files and a bind failure for
  operation and I/O cause with status 2. Check an empty certificate file
  for status 1. Assert errors omit the full manual in all cases.
- [x] 1.3 Carry the PostgreSQL operation, SQLSTATE, and typed driver cause
  through acquisition, query, and connection failures. Verify a failed
  `metadata` DBNames read reports `42P01`, the cause, a next action, and
  status 2 with empty stdout.
  Test a non-server failure and synthetic driver messages containing a
  password, DSN, SQL literal, and control and bidi characters. Check escaping
  and that these secrets do not reach stdout, stderr, or wire details.
  Check that TLS issuer and hostname failures wrapped in I/O errors produce
  distinct safe reasons and next actions.
- [x] 1.4 Route recovered REPL errors and gateway session failures to the
  safe diagnostic with their phase and scope. Verify a failed REPL query
  leaves the next query usable and a failed gateway backing session leaves
  the listener available to another client. Test the unchanged default
  client response and the explicit verbose path.
- [x] 1.5 Normalize CLI and TOML TLS choices. Verify both provider defaults,
  `PGSSLMODE` precedence, the PostgreSQL legacy opt-in pair and MSSQL trust
  alias, their TOML forms, conflict rejection before connecting, identical
  repeated modes, CA conflicts, and PostgreSQL-only `verify-ca`.
- [x] 1.6 Verify transport on controlled endpoints for both providers
  using the documented `--tls-mode disable` commands. Check encrypted
  defaults, certificate-chain and hostname checks, private CA behavior,
  and encrypted `require` without verification. Exercise direct and
  proxied connections, gateway startup, inspection, refresh and session
  reopen, and existing API defaults.
- [x] 1.7 Check that a TLS-required server rejects the explicit plaintext
  choice before the client sends a password and without a mode switch.
  Assert MSSQL PRELOGIN rejection and malformed replies send no LOGIN7.
  Verify incoming TLS and the no-TLS non-loopback guard independently
  of backing TLS.
- [x] 1.8 Update README and deployment examples for both transport modes,
  the accepted PostgreSQL legacy form, MSSQL trust compatibility, and
  incoming listener TLS. Run the documented invocations with the same
  options checked by the transport tests.

## 2. Progress and permanent warnings

- [x] 2.1 Report Config skips as resource ID and reason through all CLI
  acquisition paths, including retained, selected, and inspection reads.
  Verify a four-skip fixture prints the total and four escaped reasons
  with stderr redirected, and good resources still resolve. Check extension
  restructure warnings use the same output path.
- [x] 2.2 Accept warning serialization before decorative effects. In a
  monochrome PTY, inject a three-skip warning with a redraw pending on a
  frame ending `14.7 MiB/16.0 MiB`. Verify the whole warning and each
  ID and reason occupy separate permanent lines, and the next update
  appears beneath them. Assert the warning survives that update and the
  screen and output have no `MiBwarning:` splice. With stderr in a file,
  assert the warning and details remain whole and there are no live frames.
- [x] 2.3 Verify measured phase, percent, bytes and resources for both
  providers, known empty and unknown totals, and one completion summary
  with elapsed time. Assert alignment at 9%, 10%, and 100%. Shrink the
  PTY and verify the bar shortens first, then give it too few columns
  and verify progress suspends without affecting permanent lines.
- [x] 2.4 Register shutdown before acquisition. Send Ctrl+C in a PTY
  with a redraw and warning pending during startup and SIGHUP refresh.
  Verify one whole warning, a newline-terminated interrupted summary,
  and a stopped line with status 0. Check no repaint follows finalization,
  fatal startup returns nonzero, and failed refresh retains the listener
  and prior snapshot.
- [x] 2.5 Route access, listener, plaintext admission, and client events
  through severity-labeled logging. Verify `unrestricted access enabled`
  and `no TLS` remain text with static progress or no color, static serve
  has one outcome per attempt and empty stdout, and ordinary logs omit
  passwords.

## 3. Metadata startup effect and stdout contract

- [x] 3.1 Before changing the renderer, run `metadata` from baseline
  `f5621ca` against a deterministic data fixture and record the successful
  stdout bytes as the reference. Record the fixture, invocation, and
  revision. Cover both provider paths with fixed inputs.
- [x] 3.2 Draw the single startup glint on `metadata` in stderr while
  acquisition runs. Test an early success, warning, and fatal failure
  during the sweep. A warning or error must print immediately on its own
  permanent line. A short acquisition must exit without a pending timer
  or repaint.
- [x] 3.3 Run the revised binary on the same fixture twice, first with
  stderr on a PTY and then with stderr redirected to a file. Capture
  stdout separately in both runs. Require success and compare each
  capture with the reference using `cmp` or byte equality, including
  every space, tab and final newline. Do not strip ANSI sequences or
  normalize any bytes. Inspect the PTY for the glint and the stderr file
  for plain permanent lines without frames or control sequences.
- [x] 3.4 Record the applied REPL title-sweep baseline. The first-paint
  bracketed paste of `ВЫБРАТЬ 1;`, single submission, banner, scrollback,
  cursor, cleanup, history, and completion checks serve as regression
  coverage for the startup handoff below.
- [x] 3.5 Check scoped help in normal and narrow PTYs, `--help | less`
  with the pipe captured before the pager, and redirection to a file.
  Verify a single prominent usage and one example, task-grouped options,
  discoverable advanced and compatibility options, and no timer or
  cursor repaint.
- [x] 3.6 Check dark and light truecolor, basic ANSI, and monochrome
  rendering. Exercise `NO_COLOR`, CI, `TERM=dumb`, narrow widths, and
  stdout/stderr redirected independently. Verify the measured serve
  leading edge, static stream diagnostics, and scoped colors on labels.
  Update README examples for scoped help, metadata pipelines, and
  `TERM=dumb` as the static mode.
- [x] 3.7 Run `cargo fmt --all -- --check`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo test --workspace`, and
  `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`.
- [x] 3.8 Run `openspec validate polish-terminal-ux --strict --type change`
  and confirm the delivered behavior matches the three capability deltas.

## 4. Interactive REPL startup scenes

- [x] 4.1 Feed the startup display from the real REPL boundaries for both
  providers. Assert the sequence from `connecting` through transaction,
  layout, the applicable DBNames path, Config, the remaining acquisition
  stages, and line editor preparation. Hold each of Config, commit, and
  editor setup in turn. Assert Config counts come from totals and
  completed batches, readiness follows the editor handoff, and the
  prompt is absent while earlier work is still pending.
- [x] 4.2 Inject the motif selector and monotonic frame time internally.
  Check all three choices with deterministic input and verify the
  equal-sized sample buckets without a statistical test. Assert the
  choice survives phase transitions and resize. Compare nine fixed
  monochrome frames with the artwork and status rows in the design.
  Check the sphere label at the front, compressed at the edge, and
  hidden behind it in adjacent time samples. Keep test hooks private.
- [x] 4.3 Advance the fixed clock with frozen Config counts and with
  unknown totals. Assert that only activity changes. Advance the
  measured counts with the clock fixed and assert branch reach,
  filled-band height, or rock depth follows work. Check zero totals and
  stage completion without a fabricated ratio or overall readiness.
- [x] 4.4 Use PTYs for fast startup and input transfer. Finish before the
  first paint and assert an immediate real prompt with no frame. During
  a blocked database read send a whole bracketed UTF-8 paste, then submit
  it after the prompt. Also split a paste across the handoff. Assert
  exact input, one query per submission, normal cursor position, one
  banner, and preserved previous output, history, and completion.
- [x] 4.5 Interleave a warning and a redraw for each motif and check the
  complete newline-delimited warning and resource details remain above
  the scene after later paints. Inject a fatal connection, acquisition,
  and editor setup failure. Send Ctrl+C while loading. Check that terminal
  modes and the cursor are restored before exit, the cause or cancellation
  stays visible, no later frame or prompt appears, and exit codes match.
  Repeat warning and fatal diagnostic assertions with motion disabled.
  Keep the independent diagnostics and `MiBwarning:` checks in 1.2 and
  2.2.
- [x] 4.6 Resize an 80×24 PTY below the art's width or height and then
  expand it. Check the fitting single-row fallback, saved motif and
  counts, and prior scrollback. Test monochrome `NO_COLOR`, event-only
  `TERM=dumb`, CI, an interactive console with stderr in a file, and a
  scripted console with stderr on a PTY. Assert redirects receive plain
  diagnostics and batch stdout receives no prompt or control bytes.
- [x] 4.7 Re-run the `serve` one-row progress and warning regression and
  the standalone `metadata` stderr effect with the raw split-stream
  stdout comparisons from 3.3. Run workspace formatting, Clippy with
  warnings denied, workspace tests, rustdoc with warnings denied, and
  strict validation of `polish-terminal-ux`.

## 5. Procedural scene rendering

- [x] 5.1 Generate the branch, globe and rock at the drawable size from a stable launch seed. Review nine poses and test measured reveal separately from movement.
- [x] 5.2 Shade the scenes for truecolor, indexed, basic ANSI and monochrome terminals. Verify output on dark and light backgrounds.
- [x] 5.3 Exercise width and height changes through the renderer and a PTY, including the single-row fallback, restoration and fast handoff. Validate the change and run CLI checks.
- [x] 5.4 Render the scene surfaces in neutral tones and reserve green for the glint. Check the emitted colors across terminal palettes and the monochrome poses.

## Verification evidence

The two disposable schemas in `crates/open-sdbl-cli/tests/fixtures/terminal-ux/`
produce the same 213 stdout bytes from `f5621ca`. The PostgreSQL baseline
used `metadata postgres --host HOST --port PORT --database DB --user USER
--sslmode disable --insecure-plaintext`. SQL Server became available later,
so its baseline was captured with the preserved `f5621ca` binary using
`metadata mssql --host HOST --port PORT --database DB --user USER
--trust-server-certificate`. Both captures match the shared `f5621ca.tsv`.
Run `crates/open-sdbl-cli/tests/terminal_ux.py` against either fixture to
compare unmodified stdout with stderr on a PTY and in a file. The MSSQL
runs also passed with each of `--tls-mode disable` and `--tls-mode require`.

Live `serve` runs on both fixtures showed `Config` advance from 0 of 5
resources and 0 of 156 B to 5 of 5 resources and 156 of 156 B. The next
frame appeared after the warning and four resource details. Resizing from
100 to 55 to 20 columns shortened the bar and then hid the live row.
Each run finished with one elapsed summary and empty stdout.

PostgreSQL and SQL Server 2019 passed direct and SOCKS5 plaintext,
encrypted `require`, rejection of an untrusted chain, and certificate
verification with a matching and a nonmatching hostname. PostgreSQL also
accepted the trusted chain with a nonmatching hostname in `verify-ca`
mode. The existing MSSQL blocking API rejected the untrusted certificate
by default and connected with its existing explicit trust setting.
Gateway runs on both providers queried the inspection index, refreshed it
on SIGHUP, and reopened a lost backing session for the same client. The
MSSQL gateway requested plaintext at each PRELOGIN. The TDS test peer
separately verifies that TLS-required and malformed replies never lead to
LOGIN7.

The console startup tests compare nine monochrome poses at fixed times,
hold Config, commit and editor preparation, and inspect the final PTY
screen after warnings, failures and Ctrl+C. Run the private gate tests
with `cargo test -p open-sdbl-cli --bin open-sdbl repl::startup`.
The Linux PTY harness uses Python 3's standard library.

On the two provider fixtures, `tests/terminal_ux.py` in the CLI crate
holds the connection until a full paste or the first byte of a split
UTF-8 paste is queued. Both runs executed the query, recalled it with
Up and completed a command with Tab. Ctrl+C restored termios and exited
130. Separate runs checked the static acquisition stages, both stream
redirects, CI, and the unchanged 213-byte metadata reference.


The procedural renderer was checked against nine monochrome poses at
seed 17. For each motif, a Linux PTY held the Config counts steady and
resized from 80 columns to 120, 64, 50 and back to 80. The tests checked
art height, the single-row fallback, warning persistence and terminal
cleanup. Renderer tests separately compare fixed-time geometry after a
round trip, a second seed, and the truecolor, indexed, ANSI and plain
outputs.
