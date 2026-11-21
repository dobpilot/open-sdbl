# Design

## Context

### Reproduced on the f5621ca baseline

The baseline is `f5621ca`, built with
`CCACHE_DISABLE=1 cargo build --locked --offline -p open-sdbl-cli`.
The existing help emitted 59 stdout lines, no stderr, and status 0 both in
an 80-column PTY and with stdout captured. The following excerpts omit the
rest of the existing manual.

```console
$ target/debug/open-sdbl --help
open-sdbl — tooling for the 1C query language

Usage:
  open-sdbl lex [FILE|-]
  open-sdbl metadata postgres --host HOST --database DB --user USER [OPTIONS]

$ target/debug/open-sdbl serve postgres --wat
missing value for "--wat"

open-sdbl — tooling for the 1C query language

$ target/debug/open-sdbl serve postgres --listen 0.0.0.0:5433 --help
refusing to listen on 0.0.0.0:5433 without TLS: passwords would cross the network in clear text; pass --tls-cert and --tls-key, listen on a loopback address, or pass --allow-insecure-network

open-sdbl — tooling for the 1C query language
```

Both failing commands produced 62 stderr lines, empty stdout, and status 2.
A missing listener certificate also produced status 2 and empty stdout.
Its stderr was one line. Using `/dev/null` as the certificate gave
`TLS certificate file "/dev/null" holds no certificate` on stderr, empty
stdout, and status 1.

```console
$ target/debug/open-sdbl serve postgres --host db --database unf --user reader \
    --tls-cert /nonexistent/open-sdbl-terminal-ux.crt \
    --tls-key /nonexistent/open-sdbl-terminal-ux.key
cannot read TLS certificate "/nonexistent/open-sdbl-terminal-ux.crt": No such file or directory (os error 2)
```

The current `metadata --help` exits 0 with help on stdout and empty stderr.
Both invocations below exit 2 with empty stdout. The first appends the full
manual to the displayed error.

```console
$ target/debug/open-sdbl metadata postgres --host db
--host, --database, and --user are required

$ target/debug/open-sdbl metadata --config /nonexistent/open-sdbl-terminal-ux.toml
cannot read configuration "/nonexistent/open-sdbl-terminal-ux.toml": No such file or directory (os error 2)
```

The baseline MSSQL console rejected both `--sslmode disable` and
`--insecure-plaintext` as unknown options for MSSQL. For PostgreSQL, a
disposable server in single-user mode confirmed the error from the actual
legacy DBNames SQL against a fixture with `schemastorage` and no `params`.

```console
$ initdb -D "$ux_tmp" -A trust -U ux_review --no-locale -E UTF8
$ postgres --single -D "$ux_tmp" -c log_error_verbosity=verbose postgres
CREATE TABLE public.schemastorage (stub int);
SELECT 0::int, binarydata FROM params WHERE rtrim(filename::text) = 'DBNames';
ERROR:  42P01: relation "params" does not exist at character 32
```

Here `ux_tmp` is an empty temporary directory. The sandbox denied the local
server socket bind, so the SQLSTATE and server message were reproduced at the
SQL layer. The later connected REPL and gateway runs are recorded in
[tasks.md](tasks.md#verification-evidence).

### Reported by the operator

The supplied log excerpts contain `14.7 MiB/16.0 MiBwarning: skipped 3 unreadable…`
and `PostgreSQL query failed: db error`. The checkout explains both paths.
The reported byte totals, resource count, and exact server-side cause are
specific to that run. The `42P01` example below comes from the disposable SQL
fixture.

### Baseline entry points and output

| Surface | Current path and behavior |
| --- | --- |
| Dispatch and exits | `crates/open-sdbl-cli/src/main.rs:35` takes credentials and starts Tokio. `app.rs:24` dispatches and writes root help. `main.rs:50` flushes stdout and applies `error.rs:22` exit codes through `output.rs:13`. |
| Argument parsing | `args.rs:12` holds one help string. `args.rs:23` parses providers and options, and reads values before checking unfamiliar flags. `config.rs:14` translates typed TOML to flags. `serve.rs:61` splits and validates listener flags before `parse_connection`. |
| REPL | `app.rs:101` opens the session before `repl/mod.rs:334` initializes Rustyline and prints the welcome and prompt. `repl/terminal.rs:120` handles the Linux footer. `repl/completion.rs:395` styles editable input. |
| Metadata command | `app.rs:44` dispatches to `app.rs:74`. The handler expands config, connects, acquires the snapshot, and writes the resolution report to stderr through `output.rs:22`. `output.rs:36` emits the tab-separated stdout header and bounded `OBJECT`, `FIELD`, and `INDEX` records, with an omission trailer when applicable. |
| Gateway progress | `serve.rs:304` starts a reporter for `GatewayState::acquire` or `acquire_unrestricted`. `open-sdbl-pgwire/src/state.rs:118` passes it through to both providers' metadata acquisition. `open-sdbl-cli/src/progress.rs:38` draws with CR and erase-line, leaving the cursor at the end. |
| Current warning collision | `open-sdbl-db/src/pipeline.rs:965` collects filename and decoder error. At line 1060 it writes directly to stderr, with at most three examples. No newline or clear precedes that write when the bar is active. There is a separate direct warning for malformed extension restructures at line 93. |
| Runtime logs and shutdown | `serve.rs:208` maps gateway events to stderr text. Startup policy warnings and listening status use direct stderr writes in `serve.rs:381`. The gateway owns its signal future only after acquisition and bind. `serve.rs:334` runs a second acquisition on SIGHUP alongside live client events. |

Paths in the table without a crate prefix are relative to the CLI source
directory. Crate-qualified paths are relative to `crates/`.

`crates/open-sdbl-cli/tests/metadata_cli.rs` exercises help and required
options, invalid SOCKS5 syntax, the obsolete `--psql` flag, and the legacy
PostgreSQL plaintext diagnostic. All four tests passed on the baseline.
`src/tests/output.rs` covers escaping and layout, and `src/tests/progress.rs`
covers rendering known counters. These tests leave successful process
stdout and split-stream PTY behavior to the new acceptance tests.

## Goals / Non-Goals

**Goals**

Keep terminal ownership explicit, reuse measured acquisition facts, and
provide a static path suitable for snapshots and service logs. Neon informs
the restrained accent palette, clear hierarchy, and spacing. The text
wordmark, bar, and composition are specific to open-sdbl.
[Neon home](https://neon.com/) · [Neon Brand Guidelines](https://neon.com/brand) · [Neon on GitHub](https://github.com/neondatabase)

**Non-Goals**

Full-screen gateway dashboards, new credential sources, and changes to the
SDBL result format or wire protocol.

## Decisions

### Palette and typography

Use the terminal's default foreground and background for prose, paths,
and identifiers. Mint marks the `open-sdbl` name, the input prompt, and
the filled progress cells. In a user-specific prompt apply mint to
`=>` and leave the user name in the default foreground. On a known dark
truecolor terminal use `#34D59A` and `#B5F5DC` for the edge. On a known
light truecolor terminal use
`#08754E` and `#13805A` for the edge so the mark stays legible on white.
Use advertised truecolor and a known background hint, for example black or
white in `COLORFGBG`. Use the terminal's normal and bold green when only
basic ANSI colors or an unknown background are available. Reserve amber
and red for the short `WARN` and `ERROR` labels. Reset styling before
message text. Use blank lines and indenting for hierarchy. Plain output
keeps the same text and meaningful bar glyphs.

### REPL startup in the current checkout

`main.rs:37` takes credentials from the environment and enters Tokio.
`app.rs:25` dispatches both `console` and `repl` to `console()` after the
explicit help path. At `app.rs:126` the command expands TOML, validates the
provider and its connection options, resolves credentials, and opens the
session. The current console progress reporter starts after connection. It
ends after `session.metadata()`, before `print_resolution_report()` and
`repl::run()`. The current `paint_startup()` starts a separate title sweep
after constructing the editor. The startup scene moves that visual work
into the connection and initialization interval.

The paths in the table are relative to `crates/`. Stage labels use the
existing progress vocabulary. `line editor` uses the name from the current
editor initialization diagnostic. The table gives the producing code and
the proposed presentation.

| Event in code | Available data | What changes on screen |
| --- | --- | --- |
| `open-sdbl-cli/src/app.rs:126–145` finishes preflight and calls `DatabaseSession::connect_with_transport` | Start of the connection attempt. No measured total for socket or TLS setup, authentication, or provider setup. MSSQL also verifies the database, reads the version and year offset, and chooses the dialect in `open-sdbl-db/src/db/mssql/session.rs:286–311`. | Enter `connecting`, the initial label in `open-sdbl-cli/src/progress.rs:115`, and arm input preservation before the first connection await. |
| `open-sdbl-db/src/db/postgres/session.rs:470–484` starts the actual read-only transaction. `open-sdbl-db/src/pipeline.rs:517–529` announces the common stages. | Phase only. PostgreSQL opens its transaction before the common callback. The layout determines which DBNames path is taken. | Show `transaction` at the actual transaction start, coalescing the common label if already shown. Follow with `layout` and either `DBNames` or `DBNames (legacy layout)`. |
| DBNames returns and the `ConfigPlan::Names` branch calls `read_config` in `open-sdbl-db/src/pipeline.rs:529–532`. Both provider reads then call `config_totals` in `open-sdbl-db/src/db/postgres/metadata.rs:280–291` and `open-sdbl-db/src/db/mssql/metadata.rs:290–309`. | True `Config` entry, then resource count and compressed byte count from the totals query. | Expose the existing read boundary to the startup reporter so even the totals query is labeled `Config`. Add both denominators when the callback arrives. |
| `advance_config` and `skipped_config` in `open-sdbl-db/src/pipeline.rs:1033–1062` | Completed-batch counts and compressed bytes, including processed resources whose decoding was skipped. A skipped ID carries a decoder reason. | Update the Config counters and reveal more of the scene. Display skip summaries and details through the permanent warning writer. |
| Config returns and `open-sdbl-db/src/pipeline.rs:541–600` reports remaining phases, resolves the snapshot, commits, and calls `finish()`. | Unmeasured phases `extensions`, `SchemaStorage`, `catalog`, `resolve`, and `commit` at the read-only transaction commit. `finish()` confirms successful metadata acquisition after commit. Neither provider reports per-phase totals here. | Leave Config progress scoped to Config. Hold its completed geometry in the later phases. Use the current phase label alone. Acquiring the snapshot still leaves editor setup ahead. |
| `open-sdbl-cli/src/app.rs:147–151` gets the snapshot and writes the resolution report. `open-sdbl-cli/src/repl/mod.rs:339–359` creates the editor, installs its helper, and constructs session state. `open-sdbl-cli/src/repl/completion.rs:77` builds completion from that snapshot. | Snapshot and diagnostic findings, then editor and session preparation. No progress total. | Route each finding through the permanent writer. Show `line editor` during preparation if a render is due. |
| `open-sdbl-cli/src/repl/mod.rs:361–379` reaches the first `editor.readline(&prompt)` with its helper and state installed. | Proposed readiness event at the immediate handoff to the first editable prompt. The existing code has no separate readiness callback. | Stop rendering, clear the owned stderr rows, restore terminal ownership, and let the editor display the usable `open-sdbl=> ` prompt. |

### REPL startup scene

Select one motif per interactive launch and keep it through all phases and
resizes. Use the CLI's existing Rustls ring random source. Sample a byte
uniformly, retry on 255, and map the remaining values modulo 3. This gives
each motif 85 possible values. Inject the choice and monotonic time
internally for tests. The renderer needs no new production dependency.
Use the existing Unix terminal helpers, Tokio readiness and timing, and
`unicode-width`. Keep presentation in the CLI.

Generate the scene directly on a dot grid at the current stderr size.
Choose a variation seed once with the motif. Reuse it across frames and
resizes so forks, ridge lines and the globe axis stay recognizable. The
branch has curved, tapered limbs with smaller offshoots and bark grain.
The mountain projects a faceted height field over two ruled foundation
faces. The globe projects meridians, dotted bands and occluded labels.

Use all but the last column, up to 512 columns. Allocate one art row per
four columns, limited to the terminal height minus four rows and at most
96 art rows. These bounds limit the repaint cost for extreme PTY sizes.
Below 52 columns or 14 rows keep the fitting status line. Preserve the
sphere's round projection when height limits its diameter. Each cell
holds two by four dots. ASCII-only terminals use the same layout with
density glyphs. Sample curves and faces at the new resolution on resize.

Shade the wood, wire and stone in neutral grays up to white on a dark
background. On a light background use darker grays for the strongest
strokes. Reserve mint for the pulse along a fork, the moving sheen on
rock and the glint along a filled globe band. The dark-theme accent is
`#34D59A`. Indexed terminals use grayscale entries and one green. Basic
ANSI uses foreground intensity and a green accent. Labels and counters
inherit the terminal foreground. Monochrome uses stroke density and
hatching, with an asterisk for the glint. The
composition takes the open space of the sphere, organic forks, and the
contrast between rock and an engineered base from the references.
[Neon home](https://neon.com/) · [Neon reel](https://cdn.neonapi.io/public/images/pages/home/cta/cta-new-av1.mp4?updated=20261009) · [Toolcraft's geometry study](https://toolcraft.sh/case-studies/neon-iceberg)

Maintain an event-driven work state and a separate elapsed-time activity
phase. Show `processed / total` for Config resources and compressed bytes.
Use the compressed-byte fraction for the motif when the total is positive
and the resource fraction when byte total is zero and resources are known.
Bound the drawing fraction to the available area while reporting actual
counters. A known empty Config has zero counts and reaches completion when
the read finishes. Until totals arrive, use the phase name alone and hold
the geometry. After Config completion the full Config geometry can remain
through later phases. No clock tick, phase count, or completed Config read
indicates overall readiness. Throttle activity draws to 20 fps. The clock
only advances the pulse, rotation, or sheen.

Arm the existing non-flushing terminal handoff before the first pending
connection operation. Preserve queued UTF-8 and bracketed paste, including
a paste that spans a phase transition or the handoff itself. Keep the
terminal's input queue for the editor and switch modes without discarding
or replaying input. During loading, handle Ctrl+C as cancellation with the
guard armed, through the terminal's interrupt signal, independently of
ordinary queued input. Register the interrupt before the first await.
Keep cancellation responsive during completion indexing, where
`ConsoleHelper::from_snapshot` does synchronous work. Restore original
echo, input mode, cursor, and paste state on cancellation or failure. A
cancelled launch leaves a permanent
`Cancelled` line on stderr and exits 130. Once the editor owns input, keep
its existing interrupt behavior. Use a scoped guard and a single writer
for reservation, painting, resize, warning, and cleanup. If a cursor
region or handoff cannot be used safely, use the plain line path.

Schedule the first transient paint only if work is still pending after
120 ms. The deadline is a flash guard. It never governs work completion or
the prompt. Prioritize readiness, failure, and interrupt over pending paints.
An editor-ready launch clears the indicator and draws the compact
`open-sdbl` wordmark, the working `\help for commands` hint, and the real
prompt once. The banner is static. The editor owns the first editable line,
including its insertion cursor, history, and completion. Keep the existing
server description and fitting Linux footer in their normal places. Flush
stderr cleanup before writing the prompt to stdout, including when both
streams share a terminal.

Reserve and clear only the indicator's rows on the normal screen. A
warning clears the transient region, is printed and flushed as one
permanent line or block, and the same current state redraws below it.
Fatal errors and Ctrl+C use the same cleanup, with no later repaint.
Use explicit row boundaries and leave the last terminal column unused.
Re-evaluate the visible owned rows at resize before emitting cursor moves
for the new layout.
An undersized capable terminal uses a single status row. Keep the active
stage and one measured ratio if both cannot fit, shorten `resources` first,
and use the `DBNames` label on a narrow legacy-layout row. If the viewport
cannot fit a stage and any ratio, retain a fitting phase label and keep
permanent diagnostics readable. Clear only the visible part of an existing
owned region after a shrink. Restoring room uses the original motif,
current counters, and current activity phase. Use no alternate screen and
do not erase scrollback.

For an otherwise interactive REPL with `TERM=dumb`, print static stage
lines on transitions and a Config line when totals arrive or the stage
finishes. Print warnings as they arrive. These lines use no cursor
controls or timer repeats. CI, scripted input, redirected stdout, and
redirected stderr retain line-oriented permanent diagnostics with no
transient scene. Honor `NO_COLOR` independently. It removes colors and
preserves monochrome activity when the terminal otherwise supports it.
The `serve` bar and separate `metadata` effect keep their existing behavior.

### REPL storyboards

The nine generated panels share review seed 17 and a 72×18 cell viewport. The border is a
review guide. One fixed status row sits underneath. The middle sample
uses 31 of 48 resources and 12.0 of 24.0 MiB, so the byte-driven reveal
is halfway through its range. Read each group from top to bottom. The
ready pose is a snapshot for review and tests. At runtime the editor takes
over immediately and the prompt replaces the cleared scene.

#### Horizontal branch

Start. The branch enters from the left. Parallel cuts in its thick bark
follow the sag of the main limb. The first upward fork already has fine tips.

~~~text
+------------------------------------------------------------------------+
|                                                                        |
|                                                                        |
|                   ⢀⣤⢀⡤⢠⠋⠉                                              |
|                  ⢀⣯⣾⣅⣠⣇⢤⠖                                              |
|                ⡠⣾⡿⠋ ⠈   ⢣⡀                                             |
|              ⡠⣾⠿⠋        ⠉⠉                                            |
|**          ⣠⣾⡿⠁                                                        |
|***⣿⣶⣶⣶⣤⣤⣤⣰⣿⢟⠏                                                          |
|**⣿⣿⣿⣿⣿⣿⣿⡿⢛⣱⣏⣒⣒⣒⠤⠤⣤⣤⣀⣀⣀⣀⣀⡀                                              |
|⠛⠷⠶⢾⣿⣿⣿⣿⣿⣷⣿⣿⣿⣿⣿⣭⣛⣛⣛⡛⢿⡿⠯⢭⣭⣭⣷⡤⣀⡀                                          |
|      ⠈⠉⠉⠙⠋⠉⠛⠛⠛⠛⠿⠿⠿⠷⣽⣿⣿⣿⣿⡿⡛⠿⠛⢋⡟⠁                                        |
|                     ⠙⢿⣧⡀    ⡞                                          |
|                       ⠙⠿⡆⡠⠖⠉⠁                                          |
|                         ⠈⠳⣄                                            |
|                                                                        |
|                                                                        |
|                                                                        |
|                                                                        |
+------------------------------------------------------------------------+
open-sdbl  connecting
~~~

Middle. Half of the measured compressed bytes opens the second fork and
the pendant shoot. A short mint pulse follows a single grain line out to
the fork and fades there. Its passage leaves the visible reach unchanged.

~~~text
+------------------------------------------------------------------------+
|                                                                        |
|                                                                        |
|                   ⢀⣤**⢠⠋⠉                ⡴⠊   ⣀⠄                       |
|                  ⢀⣯⣾*⣠⣇⢤⠖               ⡸⣡⡪⠖⣢⠎⣁⡀                       |
|                ⡠⣾⡿⠋ ⠈   ⢣⡀             ⣰⠟⠛⠛⠋⠉⠉⢣                        |
|              ⡠⣾⠿⠋        ⠉⠉         ⣠⣾⠮⠁       ⢧⣀⡄                     |
|⣀⡀          ⣠⣾⡿⠁                   ⣠⣾⡞⠁                                 |
|⣿⣿⣿⣿⣶⣶⣶⣤⣤⣤⣰⣿⢟⠏                   ⢀⣴⡫⠋                ⢀⠔                 |
|⠶⢼⣿⣿⣿⣿⣿⣿⣿⡿⢛⣱⣏⣒⣒⣒⠤⠤⣤⣤⣀⣀⣀⣀⣀⣀⡀     ⣠⣿⠟              ⣀⣀⣠⣴⣋ ⣀⣀               |
|⠛⠷⠶⢾⣿⣿⣿⣿⣿⣷⣿⣿⣿⣿⣿⣭⣛⣛⣛⡛⢿⡿⠯⢭⣭⣭⣝⣒⣒⣲⣶⣾⣿⣷⣶⣶⣶⣶⣶⠿⣿⠶⠶⠶⠶⣿⣿⣿⡿⣛⣛⡻⠟⠛⠉                 |
|      ⠈⠉⠉⠙⠋⠉⠛⠛⠛⠛⠿⠿⠿⠷⣽⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣛⣛⣒⣒⠽⢿⣿⣟⠛⠛⠉⠉⠉⠉                       |
|                     ⠙⢿⣧⡀ ⠁  ⡞           ⠙⢿⣷⣄                           |
|                       ⠙⢿⡦⡠⠖⠉⠉⠓⣦⡀          ⠙⢿⣧⡀ ⣠⠚⠁                     |
|                         ⠙⢿⣦   ⢱⡀            ⠁⠙⢍⠓⢲⠒                     |
|                           ⠉⠳⣄  ⠑⠴⠂             ⠑⣌⡆                     |
|                             ⠈⢳⣦ ⣀⣀              ⠈⢿⠆                    |
|                               ⠉⠮⣀                                      |
|                                 ⠈⢳⡂⣠⠆                                  |
+------------------------------------------------------------------------+
open-sdbl  Config  31/48 res  12.0/24.0 MiB
~~~

Ready. The main limb dips and then climbs toward the right edge. Fine tips
split above and below it. Gaps between their silhouettes keep individual
shoots legible where several limbs cross the same columns.

~~~text
+------------------------------------------------------------------------+
|                                                               ⡠⠖⠂      |
|                                                              ⡼⠁⣀       |
|                   ⢀⣤⢀⡤⢠⠋⠉                ⡴⠊   ⣀⠄            ⣰⠕⠛⣣⠤⠤     |
|                  ⢀⣯⣾⣅⣠⣇⢤⠖               ⡸⣡⡪⠖⣢⠎⣁⡀           ⡴⣁⢴⠋⠁   ⡠⡀  |
|                ⡠⣾⡿⠋ ⠈   ⢣⡀             ⣰⠟⠛⠛⠋⠉⠉⢣          ⢀⣮⠚⠁ ⠙⠦⢤⡴⠿⡎   |
|              ⡠⣾⠿⠋        ⠉⠉         ⣠⣾⠮⠁       ⢧⣀⡄      ⡠⠊         ⠘⡄  |
|⣀⡀          ⣠⣾⡿⠁                   ⣠⣾⡞⠁               ⢀⣼⡿            ⠈⠓⠒|
|⣿⣿⣿⣿⣶⣶⣶⣤⣤⣤⣰⣿⢟⠏                   ⢀⣴⡫⠋                ⣠⣷⠋      ⢀⣀⡀⣀⣀⡤⠤⠔⠒⠒|
|⠶⢼⣿⣿⣿⣿⣿⣿⣿⡿⢛⣱⣏⣒⣒⣒⠤⠤⣤⣤⣀⣀⣀⣀⣀⣀⡀     ⣠⣿⠟              ⣀⣀⣠⣾⣿⣁⣤⣤⣤⣶⣶⣿⣿⠿⠛⠋       |
|⠛⠷⠶⢾⣿⣿⣿⣿⣿⣷⣿⣿⣿⣿⣿⣭⣛⣛⣛⡛⢿⡿⠯⢭⣭⣭⣝⣒⣒⣲⣶⣾⣿⣷⣶⣶⣶⣶⣶⠿⣿⠶⠶⠶⠶⣿⣿⣿⡿⣛⣛⣛⡫⠭⠛⠋⠉⠉  ⠙⠻⢧⡀    ⢀⠔  |
|      ⠈⠉⠉⠙⠋⠉⠛⠛⠛⠛⠿⠿⠿⠷⣽⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣛⣛⣒⣒⠽⢿⣿⣟⠛⠛⠉⠉⠉⠉              ⠉⢢⡤⠤⠴⠭⣄⡀ |
|                     ⠙⢿⣧⡀ ⠁  ⡞           ⠙⢿⣷⣄                    ⠉⢢⡀ ⢣  |
|                       ⠙⢿⡦⡠⠖⠉⠉⠓⣦⡀          ⠙⢿⣧⡀ ⣠⠚⠁                ⠙⣆⠈⠣⠴|
|                         ⠙⢿⣦   ⢱⡀            ⠁⠙⢍⠓⢲⠒                 ⠘⢷⣄⢠|
|                           ⠉⠳⣄  ⠑⠴⠂             ⠑⣌⡆                  ⠈⢢⡇|
|                             ⠈⢳⣦ ⣀⣀              ⠈⢿⣦⠆                  ⠙|
|                               ⠉⠮⣀                 ⠑⢦⡴⠃                 |
|                                 ⠈⢳⡂⣠⠆               ⠉⠓⠒⠂               |
+------------------------------------------------------------------------+
open-sdbl=>
~~~

#### Sphere

Start. A tilted polar axis and bowed meridians establish the volume.
Three dotted latitude bands wrap the near face. The SDBL label has its
full letter spacing as it crosses the central meridian.

~~~text
+------------------------------------------------------------------------+
|                              ⢀⣀⣤⣤⣤⣤⣤⣤⡤⢤⣀⡀                              |
|                          ⢀⣤⠶⡻⠛⢹⡈⠉⠑⠒⢄⡀⢘⣩⠵⠛⢽⡒⢤⡀                          |
|                        ⣠⣾⠓⢁⠜   ⢇⣀⣀⠤⠔⠊⠛⣄ ⠄ ⠈⠑⢎⡳⣄                        |
|                      ⣠⣮⣚⣐⡠⡮⠤⠖⠒⠊⠹⡁     ⠈⠲⡀  ⠄⢠⡭⢿⡳⣄                      |
|                     ⡴⡱⠃⢀ ⣸      ⢣     ⠄⠠⣘⣥⠗⠚⠛ ⠢⠘⣍⢦                     |
|                    ⡼⡱⠁   ⡇      ⠜⡄⢈⣀⠶⠔⠚⠍⠡⡀⢣⠈⠁   ⠈⢎⣧                    |
|                   ⢰⢳⠡⡀⠠⢀⢀⠇l e x⠒⠪⢫⠉⠐ ⠒     ⢳   ⣀⢤⠚⣯⡆                   |
|                   ⡿⡟⠖⠓⠎⡉⢹⠋⠠⡀⠈ ⠒  ⠈⡇       ⢀⣠⢧⠒⠯⢁⡀⠁⠻⣿                   |
|                   ⡗⡇⠈⠂ ⠈⠘⡄        ⢸ ⢀⣀⢤⠔⠒⠋⠉⠐⠸⡦⠠⡀ ⠉⠐⣿                   |
|                   ⡇⡇    ⠈⡇  ⢀⣀⡠⢤⠤⠒⠒⡏⠁⡀ ⠈ ⠑ ⠐⠄⢱   ⣀⠔⣿                   |
|                   ⣗⡗⠤⡔⠒⠲⡒⢻⠉⠋⠅⢀  ⠁S D B L⠁   ⢀⡼⡴⠒⠉ ⠠⣿                   |
|                   ⠸⣻⠢⢁⠐⠄⡀⠈⡇ ⢄ ⠁ ⠑   ⢣ ⢀⣀⠤⠔⠒⠉⠁ ⡇   ⢰⠇                   |
|                    ⢳⣧ ⠁  ⠁⢱   ⢀⣀⣀⠤⠔⠒⠚⡍⠁     ⢀ ⡇⣄⡬⢾⡟                    |
|                     ⠻⣶⠤⠤⠒⠒⠒⢳⠉⠉⠁      ⢣ ⢀Config⠭⡁⢙⠞                     |
|                      ⠙⢵⡂    ⠱⡀⠠  ⡀⢀⣀⣐⠬⡶⠲⠎⣉⠈⠡⠑⡎⢠⡴⠋                      |
|                        ⠙⢶⣤⢤⢦⡦⠼⢖⡓⡒⠙⠍⠠⠄⠈⢹  ⢀⣀⣤⢜⡾⠋                        |
|                          ⠈⠓⠮⣆⡈⢁⠑⠤⣉⣀⣀⣠⠤⠜⣗⢉⣩⠴⠛⠁                          |
|                              ⠈⠉⠛⠛⠛⠛⠒⠒⠒⠚⠉⠁                              |
+------------------------------------------------------------------------+
open-sdbl  connecting
~~~

Middle. Rotation carries SDBL to the right limb. Its four characters
close up against the contour. The measured half-fill packs the lower
ribbon and starts the equatorial one. With frozen counts the label
continues around the limb while band density and the status row hold.

~~~text
+------------------------------------------------------------------------+
|                              ⢀⣀⣤⣤⣤⣤⠤⠤⠤⢤⣀⡀                              |
|                          ⢀⣤⢶⠿⢻⠋⠉⠺⣄⡀⠉⠉⢒⣤⢴⡋⠙⠲⢤⡀                          |
|                        ⣠⠾⢉⠞⡁ ⡎  ⣀⣠⠵⡖⠚⠉  ⠉⠒⢤ ⠉⠳⣄                        |
|                      ⣠⣞⣁⣰⡣⠤⠤⠖⡗⠊⠉⠁  ⠈⢆      ⠝⢬⣨⠞⠷⣄                      |
|                     ⡴⠃ ⡰⠁  ⠂ ⡇       ⣣⠄⠠lex⠚⠛⠑⡅⠉⠈⢦                     |
|                    ⡼⢁ ⢠⠃     ⢇ ⠄⢀⡀⢈⣀⠶⠔⢳⠍⠠⡀⠁⠈⠁ ⠘⢄ ⠈⣧                    |
|                   ⢰⠇⢄ ⣜ ⠐⠠⣀⣊⡠⢾⠔⢖⠊⡉⠉⠐ ⠒⡀⢣       ⣉⡶⢞⠫⡆                   |
|                   ⣯⠹⡚⠲⣏⠉⠫⠩⡀⠈ ⠺⡄  ⠈     ⠈⡆ ⢀⡠⠤⠒⠯⠁⡹⡢⢍⢿                   |
|                   ⡏ ⠑ ⡇⠁    ⠂ ⢇     ⢀⣀⠤*⠚⣍⠁⣈⠁⡐⠂SDBL⢸                   |
|                   ⡇ ⢀ ⡇     ⢀⣀⣸⣤⠤*⠺⡉*⡈⠐⠁⠂⢵⠠⠄ ⠈  ⢀⣸⠴⢻                   |
|                   ⣷⢒⠦⡔⡗⢖⠒⠺⡉⢉⠃⠔⠠⢇⠁⡀⠒ ⠄⠌ ⠈⠁⠈⡆ ⢀⡠⠔⠲⠉⢸ ⣸                   |
|                   ⠸⡪⢐⠌⣸⠄⢀⠑ ⢄⠁ ⠢⠸⡀⠈    ⢀⣀⠥⠔⢳⠉⠁    ⣸⣴⠇                   |
|                    ⢻⡀⠁⠉⢇ ⠁    ⢀⣂⣳⠤⠤⠒⠒⠉⠁   ⢸  ⢀⢤⣔⣾⣿⡟                    |
|                     ⠳⡲⠤⠼⢖⠒⠒⠒⠉⠉⠁ ⠈⢆     ⢀⠨⡠⣺⣲⣝⢗⣽⡻⣟⠟                     |
|                      ⠙⢄ ⠈⢣⡀    ⢀⡀⣝⣆⣤⣢⢾⡼⣳⢯⢟⢽⠕⠑⠍⢀⡮⠋                      |
|                        ⠙⢶⣶⣷⣾⡧⣿⡞⣷⢻⣝⢿⣷⠏⠞⠙⠁⠉⢀⣸⡤⢔⡾⠋                        |
|                          ⠈⠛⠿⣟⡻⣅⡙⠁⢁⣀⣈⣢⡤⠔⢒⢋⣽⠧⠚⠁                          |
|                              ⠈⠉⠛⠛⠛⠒⠓⠒⠚⠛⠉⠁                              |
+------------------------------------------------------------------------+
open-sdbl  Config  31/48 res  12.0/24.0 MiB
~~~

Ready. SDBL has rotated out of sight and serve crosses the near face.
DBNames is visible on the lower band. The time samples are 0°, 57° and
134° into a four-second turn. Sparse far-side dots and clear space near
the poles keep the meridians visible even with all three ribbons filled.

~~~text
+------------------------------------------------------------------------+
|                              ⢀⣀⣤⣤⣤⣤⣤⠤⠤⢤⣀⡀                              |
|                          ⢀⣤⣖⠟⠋⡇⠉⠑⡢⣀ ⠉⢉⣲⠶⣏⠙⠒⢤⡀                          |
|                        ⣠⠾⢉⡔⠁  ⡇ ⣀⣀⠬⠷⡚⠉   ⠉⠶⢄⠈⣳⣄                        |
|                      ⣠⣞⣉⣀⡾⠤⠤⠔⠒⡏⠉⠁ ⠂ ⠈⢢⡀  ⢀⢀⣔⣿⣿⣿⢷⣄                      |
|                     ⣴⠏  ⡎⠠    ⡇     ⢀⢀⡵⣶⢘⣿⢼⣿⢩⢯⠿⡛⠈⢦                     |
|                    ⣼⡋  ⣸    ⢀⢀⢱⡠⡄⣢⣒⣽⡬⢷⢛⡽⣃⢞⠔⠓⠈⠁ ⠘⡄⠈⣧                    |
|                   ⢰⣗⣤⣤⢤⡷⢲⢔⣖⣕⣵⢵⡽⡞⣪⢛⠱⡩⠜⠅⠃⠊⠸⡀    ⠁⣀⣼⣾⣿⡆                   |
|                   ⣿⣿⡿⢾⣫⢟⡯⡙⢽⡢⡑⠕⠌⡏⠂⠁⠁ ⠈    ⢱⣀⣀⣤⠲⡋⢯⢮⢿⣿⢿                   |
|                   ⡏⠚⠓⠓⢸⠁⠉⠈ ⠄   ⢸    ***⡔⠖⡍⢧⠱⡌⢮⣙⢕⠕⠙⡇⢸                   |
|                   ⡇   ⠈⡆    ⢀⣀⣠*****⢣⢑⡩⣐⠕⡊⢞⡵⠊⠂⠁  ⣠⡷⢻                   |
|                   ⣷⢖⣧⣴⢒⣗⢖⢶⢙⠝⡕⡌⢎⢔⠺s e r v e ⢇⣀⣠⠤⠚⠉ ⡇⣼                   |
|                   ⠸⣿⣿⣝⢷⣽⢅⠳⡱⡕⡌⠎⠆⠅⠊⢇⠁   ⢀⣂⠤⠴⠒⢹⠁    ⢀⣷⠇                   |
|                    ⢻⡉⠉⠁⠉⢫⠁    ⢄⣀⣀⠼⡤⠒⠊⠉⠁    ⠸⡀⣀⣤⣴⣼⣿⡟                    |
|                     ⠳⡒⠤⠤⠚⢖⠒⠒⠉⠉⠉   ⢱⡀   ⡐⣀⢤⣔⣴⠷⣾⢟⡽⣿⠟                     |
|                      ⠙⢆⡀⠁ ⠣⡀   ⣀⢂⡠⡠DBNames⡷⢿⠻⠉⢁⡴⠋                      |
|                        ⠙⢶⣦⣶⣿⢼⣧⢻⡞⣾⣻⣟⠮⢟⠜⠓⠉⠁⢀⣀⡮⢔⡾⠋                        |
|                          ⠈⠛⠿⣟⡛⠮⣍⠈ ⣀⣀⡨⢧⡔⢚⢋⣩⠿⠚⠁                          |
|                              ⠈⠉⠛⠛⠛⠒⠓⠒⠒⠛⠉⠁                              |
+------------------------------------------------------------------------+
open-sdbl=>
~~~

#### Rock and foundation

Start. The camera picks out the central summit and its two shoulders.
Broken cuts follow the folds on the near slope, changing direction at
the ridge. The lower part of the block is below the viewport.

~~~text
+------------------------------------------------------------------------+
|                                                                        |
|                                                                        |
|                                                                        |
|                                                                        |
|                                                                        |
|                                                                        |
|                                                                        |
|                                                                        |
|                                                                        |
|                                   ⠈⣀⠄⣦⣄⡂                               |
|                    ⣀⠤⠢⣄⡀       ⡀⠤⠒⠉⡀⠓⣦⣍⡻⡷⣦⣄⡀                           |
|                   ⠈⣀⡤⢦⣍⡛⢷⣤ ⡀ ⠂⠁⣀⠤⠒⠉⡉⠓⣷⣍⡛⠿⣦⣌⡛⠷⣦⣄⡀                       |
|                ⡀⠐⠒⠉⣀⡤⢮⣮⡛⠷⢷⠋⣁⠄⠢⡍⣀⠤⠖⠋⡉⠓⣦⠌⣉⡷⢦⣍⡻⡷⢦⣝⣟⠤⠶⣦⡀                   |
|            ⣀⠄⠂⣅⡉⠓⠒⠋⣁⡤⠦⣄⡙⠳⣦⠉⣀⠒⠦⡌⡁⠒⠶⡋⡉⠄⠒⠉⣁⠤⢲⣌⡛⠻⠦⣌⡙⠳⢦⣌⡙⠓⢦⣌⡐⢦⢄             |
|        ⣀⠄⠂⠉⣀⡔⠦⣄⡉⠓⠶⡋⣁⠤⠢⣄⣙⠧⠒⠉⣁⠓⠦⢀⡉⠓⠦ ⣀⠤⠒⠉⣁⠤⢢⣌⡙⢷⣦⣍⡻⡳⢦⣝⣝⠳⢦⣬⡙⠳⢶⣄⡀           |
|    ⢀⡠⠔⠉⣀⠄⢂⡁⣁⡔⠦⢀⡉⠓⠦ ⣀⠤⠦⣄⣁⠤⠒⠋⡁⠧⠒⠉⣙⠥⠒⠉⣀⡤⠒⠉⣀⡤⠮⣌⡙⢳⡷⠭⢟⡻⢦⣌⡙⠳⢦⣌⡙⠳⣦⣌⡙⢷⠦⣄⡢  ⡀    |
|    ⢨⠭⣒⣛⡒⠋⠁⠈⠑⠤⢖⣁⣉⣥⡦⣏⡀⡤⠦⠄⡁⠤⠒⠋⡁⠄⠂⠁⣀⠤⠒⠉⣁⡤⠒⣉⣀⡤⠦⢄⠝⠁   ⠉⠉⠙⠛⠳⠶⣋⣝⠳⢮⣮⡹⠧⠓⢉⣉⣉⠭⡅    |
|    ⢸⣀  ⡏⠉⠉⢲⠒⠢⠤⡤⣀⣀⠉⠑⠫⠷⣦⣍⣈⡧⠖⠉⣀⠤⠒⠉⣀⠤⠒⠉⣁⡤⢦⣄⣁⢄⠔⠁          ⣀⣀⢭⠭⠕⠒⡖⠉⠉⢹  ⣀⡇    |
+------------------------------------------------------------------------+
open-sdbl  connecting
~~~

Middle. Half the measured bytes moves the same terrain up four rows.
The irregular rock seam gives way to a split foundation. A mint sheen can
cross an exposed grid edge while the camera holds this exact elevation.

~~~text
+------------------------------------------------------------------------+
|                                                                        |
|                                                                        |
|                                                                        |
|                                                                        |
|                                     ⢀⡀                                 |
|                                   ⠐⠉⡀⣌⡑⠤⡀                              |
|                     ⣀⢄⡀         ⣀⠤⠒⠉⠦⣍⡛⢷⣦⣄⡠⡀                           |
|                 ⡀⠄⠒⠉⣀⣌⡛⠷⣦⣄   ⠄⠂⠁⣀⠤⠖⠓⠦⣯⡛⠷⣷⣍⡛⠷⣦⣄⡀                        |
|                 ⠤⠤⠒⠉⣁⣜⣝⠷⢮⣯*⠋⡀⢄⡒⠉⣀⡤⠖⠓⠆⣍⠙⠓⣢⣍⡛⢷⣦⣍⡻⡷⡁⣄⣀                    |
|             ⡀⢄⡂⠓⠦⠤⠖⠋⣁⢌⡉⠳⢦⣌*⠉⠦⣄⡑⠋⠧⣤⡖⠃⡀⠥⠒⠋⣁⢬⡛⠷⣷⣌⡙⠻⢦⣄⡙⠳⠦⣌⡐⠲⣤⣀             |
|         ⡀⠄⠒⠁⡠⣄⡉⠓⠦⣤⠖⠋⣁⢄⡉⠳⣂**⠋⠦⡄⠁⠓⠦⡄⠁⠁⣀⠤⠒⠋⣀⢌⡙⠳⣦⣄⡛⢷⢦⣌⡻⡳⢦⣄⣍⠳⢮⣬⡁⠠           |
|     ⣀⡤⠒⠉⡀⠄⠂⠋⡡⡄⠉⠓⠦⡄⠝⠉⣀⣄⡉⠋⣀**⠋⣂⠤⠚⠓⣂⠤⠒⠉⣀⠤⠒⠉⣀⣌⡙⠳⢮⣏⠿⣷⢷⣌⡙⠿⢦⣌⡙⠳⢦⣌⡙⠳⣦⣅⡀⢤⠄⣀     |
|    ⢨⣑⣒⡒⠛⠊⠉⠉⠛⢥⣤⣊⣓⣀⣄⡒⠉⣁⣄⡉⠋⢀**⠁⡀⠄⠒⠉⣀⠤⠒⠋⣁⠤⠒⠋⣁⣄⣉⠷⠉   ⠉⠙⠛⠓⠦⢬⡗⡳⢦⣝⢝⣳⠮⠔⠛⢊⣉⣉⡅    |
|    ⢸  ⠈⡏⠉⠒⢲⠢⠤⠤⣀⣀⡈⠉⠛⠷⢦⣄⣒⣑⣂**⠉⣀⠤⠒⠉⣀⠤⠒⠋⣁⣄⡉⠋⣁⠔⠁          ⢀⣈⣉⠭⠭⠕⡖⠒⠉⢹⠁  ⡇    |
|    ⢸⠉⠉⠑⡗⠒⠤⢼⢄⣀⣀⡇ ⠈⢹⠉⠒⠒⡤⠤⢄⣉⣉⠒⠯⠤⠤⠶⠭⠤⣤⡖⢋⣅⣮⡹⠟⠁   ⣀⣀⡠⠤⢤⠒⠒⠉⡏⠁ ⢸⣀⣀⡠⡧⠤⠒⢺⠊⠉⠉⡇    |
|    ⢸⣒⣤⡤⣇⣀⣀⢸   ⡏⠉⠑⢺⠒⠤⠤⣇⣀⣀⢸ ⠉⠉⡗⠒⠒⢤⠤⢄⣈⣁⡠⠤⡤⠒⠒⢺⠉⠉ ⡇⣀⣀⣸⠤⠤⠒⡗⠊⠉⢹   ⡇⣀⣀⣸⢤⣤⣒⡇    |
|    ⢸⠃⠃⠏⡏⢷⢲⢻⢭⢶⣖⣧⣤⠤⣸⣀⡀ ⡇  ⢹⠉⠒⠒⡧⠤⠤⣸⣀⡀ ⡇⢀⣀⣇⠤⠤⢼⠒⠒⠉⡏  ⢸ ⢀⣀⣇⠤⣤⣼⣲⡶⡭⡟⡖⡾⢹⠹⠘⠘⡇    |
|    ⢸⣑⠒⠒⡧⠤⢄⣸⣈⠈⠃⡇⠃⠏⢿⢱⢺⢻⣯⡶⣖⣺⣤***⡀ ⢸ ⠈⠉⡏⠁ ⡇ ⢀⣸⣀⠤⣤⣗⣲⢶⣽⡟⡗⡎⡿⠹⠘⢸⠘⠁⣁⣇⡠⠤⢼⠒⠒⣊⡇    |
+------------------------------------------------------------------------+
open-sdbl  Config  31/48 res  12.0/24.0 MiB
~~~

Ready. The full eight-row rise exposes the bottom corner. The two grid
faces meet at the front vertical edge. A paired course line runs across
both faces, with short incisions below the doubled line.

~~~text
+------------------------------------------------------------------------+
|                                                                        |
|                                   ⠐⠈⡀⣌⡐⠄⡀                              |
|                     ⣀⢄⡀         ⣀⠤⠒⠉⠦⣍⡛⢷⣦⣄⡀⡀                           |
|                  ⠄⠒⠉⣀⣄⡛⠷⣦⣄⡀  ⠄⠂⠁⣀⠤⠒⠓⠦⣯⡛⠷⣷⣍⡙⠷⣦⣄⡀                        |
|                 ⠠⠤⠒⠉⣁⣜⣝⠷⢮⣯⠖⠃⡀⢄⡒⠉⣀⡤⠖⠓⠆⣍⡙⠓⣦⣍⡛⢷⣦⣍⡻⡷⣀⣤⣀                    |
|             ⡀⢄⡂⠓⠦⠤⠖⠋⣁⣌⡉⠳⢦⣌⠒⠉⠤⣄⡑⠋⠦⣤⠖⠓⡀⠭⠒⠋⣁⢬⡛⠷⣷⣌⡙⠻⢦⣄⡙⠳⠦⣄⡐⠠⣄⡀             |
|         ⡀⠄⠒⠉⡠⣄⡉⠓⠦⣤⠖⠋⣁⢄⡉⠳⣂⠥⠖⠋⠦⡄⠁⠓⠦⡄⠁⠁⣀⠤⠒⠋⣀⢌⡙⠳⣦⣄⡛⢷⢦⣌⡻⡳⢦⣄⣝⠳⢮⣬⡐ ⡀          |
|     ⣀⡤⠒⠉⡀⠄⠂⠋⡡⣄⠉⠓⠦⡄⠁⠉⣀⣄⡉⠋⣀⠤⠖⠋⣆⠤⠚⠳⣂⠤⠒⠉⣀⠤⠒⠉⣀⣌⡙⠳⢮⢯⢟⡷⢷⣌⡙⠻⢦⣌⡙⠳⢦⣌⡙⠳⣦⣄⡀⢤ ⡀     |
|    ⢈⣓⡢⠦⠛⠒⠊⠑⠫⢅⣤⠊⠓⣂⣄⡖⠉⣀⣄⡉⠋⢀⠤⠖⠁⡀⠄⠒⠉⣀⠤⠒⠋⣁⠤⠒⠉⣁⣄⡉⡳⠊⠁ ⠈⠑⠚⠛⠷⠦⣬⠟⡳⢦⣝⣝⣳⡮⠶⠛⠓⢊⣉⡁    |
|    ⢸ ⠈⠉⡏⠒⠒⢢⠤⠤⣀⣉⡉⠉⠑⠫⠷⣧⣄⠒⠑⣆⡤⠒⠉⣀⠤⠒⠉⣀⠤⠒⠋⣁⣄⡉⠋⣁⠔⠊           ⢉⣉⣉⠭⠭⡔⠒⠒⢹⠉⠁ ⡇    |
|    ⢸⠉⠑⠒⡗⠤⠤⢼⣀⣀ ⡇⠈⠉⢹⠒⠒⠢⡬⠭⣉⣉⠒⠲⠯⣀⣤⣒⣉⣀⣤⠖⠋⠁⣮⣙⠟⠁    ⣀⣀⠤⢤⠔⠒⠒⡏⠉⠁⢸ ⣀⣀⡧⠤⠤⢺⠒⠊⠉⡇    |
|    ⢸⣤⡤⢄⣇⣀ ⢸  ⠉⡏⠑⠒⢺⠤⠤⢄⣇⣀ ⢸⠉⠉⠑⡖⠒⠤⢤⢄⣀⣉⣉⣉⡠⡤⠤⠒⢲⠊⠉⠉⡇ ⣀⣸⡠⠤⠤⡗⠒⠊⢹⠉  ⡇ ⣀⣸⡠⢤⣤⡇    |
|    ⢸⠃⠏⠏⡏⣶⢻⢻⢵⢖⣒⣧⠤⣀⣸⡀  ⡇ ⠉⢹⠑⠒⠒⡧⠤⣀⣸⡀  ⡇ ⢀⣇⣀⠤⢼⠒⠒⠊⡏⠉ ⢸  ⢀⣇⣀⠤⣼⣒⡲***⣶⢹⠹⠹⠘⡇    |
|    ⢸⠒⠒⠤⡧⢄⣈⣸⠈⠘⠃⡇⠏⠏⢿⢺⢻⢭⣷⣖⣢⣼⠤⣀⣀⡇  ⢸⠈⠉⠉⡏⠉⠁⡇  ⢸⣀⣀⠤⣧⣔⣲⣾⡭⡟⡗⡿⠹⠹⢸⠘⠃⠁⣇⣁⡠⢼⠤⠒⠒⡇    |
|     ⠉⠉⠑⠓⠒⠤⠼⢍⣉⣑⡗⠒⠤⢼⢄⣈⣈⡏⠃⠃⢻⠏⠿⢱⣿⢻⢭⣾⣖⣢⣤⣧⣔⣲⣷⡭⡟⣿⡎⠿⠹⡟⠘⠘⢹⣁⣁⡠⡧⠤⠒⢺⣊⣉⡩⠧⠤⠒⠚⠊⠉⠉     |
|               ⠉⠉⠑⠚⠒⠤⠤⢏⣉⣑⢺⠒⠤⠤⣇⣈⣈⢹⠃⠃⠃⡟⠘⠘⡏⣁⣁⣸⠤⠤⠒⡗⣊⣉⡹⠤⠤⠒⠓⠊⠉⠉               |
|                         ⠉⠉⠒⠒⠣⠤⠤⣹⣉⡒⠒⡗⢒⣉⣏⠤⠤⠜⠒⠒⠉⠉                         |
|                                  ⠈⠉⠉⠁                                  |
+------------------------------------------------------------------------+
open-sdbl=>
~~~

### Explicit help

Root help in its single paint. Usage is the strongest line after the title.

```text
open-sdbl  Tools for the 1C query language

Usage
  open-sdbl <command> [options]

Commands
  lex        Print lexical tokens
  metadata   Read 1C metadata
  console    Run 1C queries interactively
  serve      Serve SDBL over the PostgreSQL protocol

Example
  open-sdbl console --config base.toml

Run open-sdbl <command> --help for command options.
```

Use an inline mint gradient across the title, with a brighter pair of
letters near the center. This is a single frame with no timer, cursor
motion, or repaint. Render the page through the existing help dispatch
and stdout writer using scoped sections in place of the shared full-manual
string. Use bold terminal-default type for the usage line and wrap narrow
descriptions under their labels. `open-sdbl --help | less` and
`open-sdbl --help > help.txt` receive a plain page. The command and
provider pages group options by task and include infrequent options
under `Advanced` and `Compatibility`.

### Metadata command

For `open-sdbl metadata --config base.toml > snapshot.tsv` with stderr
still attached to a terminal, the following progress states share one
stderr row. The completed row and any warnings stay in history.

```text
open-sdbl  metadata  connecting
metadata  Config  [===========>----]   75%   90/120 res  12.0 MiB/16.0 MiB
metadata  complete  120 resources  16.0 MiB  1.4 s
```

After argument validation, start one mint sweep over the word `metadata`
on terminal stderr, using a two-character glint and a 320 ms cap.
Advance acquisition and the sweep concurrently from the first frame.
Phase changes and measured progress can appear during the sweep. A warning
immediately clears the live row, writes a permanent line or warning block,
and lets the latest row resume below it if work is still active. A fatal
error clears and finalizes the row before the permanent diagnostic. A
fast run may show only the initial frame and finishes as soon as the work
does.

Keep serialization and stdout flushing on the current path. For acceptance,
use a deterministic snapshot fixture for the same CLI invocation on baseline
`f5621ca` and the implementation. Capture the baseline's successful stdout
as the byte reference. Compare separately captured stdout against it with
stderr on a PTY and again with stderr in a file. Check the header, records,
spaces, tabs, escaping, and final newline by comparing raw bytes. Any stray
ANSI sequence also fails that comparison. Redirected stderr contains
only permanent diagnostics in their plain line format.

### Destination and terminal policy

| Destination | Behavior |
| --- | --- |
| Capable stdout TTY | Inline help styling. Interactive REPL also needs stdin on a terminal and keeps editing, completion, highlighting, and the fitting Linux footer. |
| Stdout pipe or file | Static help text, original lexer and snapshot records, and the existing batch-console data format. The console reads with its bounded input path and omits the greeting, prompt, and footer. |
| Capable stderr TTY | The startup scene is available for interactive REPL, with a single-row fallback sized in display columns. The `serve` and `metadata` commands use a live line. Permanent logs and diagnostics remain separate from the transient region. |
| Stderr pipe or file | Plain warnings and errors. `serve` additionally emits a single acquisition outcome per attempt. `metadata` and `console` suppress decorative progress. |
| CI or unsupported terminal | Static for both streams, also with a PTY. Treat nonempty `CI` as CI. `TERM=dumb` turns off motion and allows a plain event-driven startup transcript when the REPL streams are terminals. Unknown geometry disables cursor animation. |
| Nonempty `NO_COLOR` | Disable color, including Rustyline token accents and the `metadata` title sweep. Keep permitted monochrome progress, REPL activity, and command editing. An empty value behaves as unset. |

Detect each stream independently, including when stdout is redirected and
stderr is still interactive. Check viewport geometry using the existing
terminal helpers and `unicode-width`. Respect the existing `COLUMNS`
fallback for result table sizing. The pinned Rustyline already treats
`TERM=dumb` as an unsupported editor terminal. Apply the same setting to
the REPL footer, help, and stderr renderers. Output redirection also
suppresses motion on the redirected stream. Palette, duration, and bar
shape are fixed presentation defaults.

Use the dependencies already in `crates/open-sdbl-cli/Cargo.toml`.
Keep `rustyline 18.0.1` for editing and paste, Unix `libc` for terminal
modes, geometry, and input readiness, `unicode-width` for column
measurement, and Tokio timers and signal handling for acquisition and
redraw deadlines. One serialized writer and short inline ANSI spans
cover the design.

### Progress and permanent stderr

The CLI owns one stderr writer for a process. Within one serialized
operation, clear the active region, write and flush the entire
newline-terminated message or warning block, and redraw the latest frame
below it if work is pending. `serve` and interactive `metadata` leave a
permanent outcome. The REPL clears its startup region before the editor
shows the prompt. Acquisition workers send events to this writer.
Reuse the same path for startup, SIGHUP, gateway events, and terminal errors.
Use plain `INFO` for connection and listener lifecycle, amber `WARN` for
refusals, skips, and security policy, and red `ERROR` for failed operations.
Throttle ordinary progress repainting to one frame per 50 ms, with immediate
phase transitions, restoration after a log, and final outcomes.

Carry resource ID and a safe decoder reason through an additive reporting
callback. Collect skipped IDs from completed batches, then emit a summary
and a line per resource in stable ID order at the end of the Config phase.
Flush already-reported skips on an error or interruption. Route the
extension restructure warning through the same writer. Explicitly carry
the reporter through retained and selected Config reads that currently use
`NoProgress` while decoding, including the gateway inspection path. In
retained reads the acquisition pass accounts for bytes and resources once,
while the decoder forwards warnings only. Embedded callers can receive
the same events without terminal I/O.

Label the current stage and use the existing provider `config_totals` and
`advance_config` values. The percent measures compressed Config transfer
or processing and uses resources when known bytes are zero. Show unknown
totals and percentages as `?` and distinguish known empty transfers. A
deliberately skipped resource counts as processed. Config can reach 100%
while metadata resolution, commit, users, rights, or catalog work continues.
Keep measured counters through finalization and report elapsed time from
a monotonic clock when the acquisition outcome is known.

Reserve four columns for the right-aligned percent including `%`, size
the resource counters from their known total, and hold a common byte unit
and digit width for the phase once totals arrive. The bar and other fields
stay aligned across 9%, 10%, and 100% at stable terminal width. Use
`=` for completed cells, `>` for the leading filled cell, and `-` for
remaining cells. Color the fill mint and brighten the leading cell. Move
the edge only when measured progress reaches a new cell. On resize shorten
the bar from 16 cells to six first, then compact labels such as
`resources` to `res`. If the phase and counters still cannot fit in one
row with a spare column, suspend live output and restore it only when it
fits. Permanent lines can wrap normally.

Three serve frames show the handling of the reported
`14.7 MiB/16.0 MiBwarning: skipped 3 unreadable Config resources` boundary.
The warning and byte boundary come from the operator's log. The positions,
IDs, reasons, and resumed measurements are illustrative.

Frame 1. Config reading, with the security notices already in history.

```text
WARN  unrestricted access enabled; authenticated users bypass role restrictions.
WARN  no TLS on the listener; plaintext passwords allowed from loopback clients.
metadata  Config  [=============>--]   92%  115/120 res  14.7 MiB/16.0 MiB
```

Frame 2. The warning arrives with a redraw pending. Clear the dynamic row
and commit the warning and its resource details to history.

```text
WARN  unrestricted access enabled; authenticated users bypass role restrictions.
WARN  no TLS on the listener; plaintext passwords allowed from loopback clients.
WARN  skipped 3 unreadable Config resources
      11111111-1111-1111-1111-111111111111.7  invalid UTF-8
      22222222-2222-2222-2222-222222222222.7  invalid UTF-8
      33333333-3333-3333-3333-333333333333.7  invalid brace serialization
```

Frame 3. The warning stays in history and the latest acquisition frame
resumes one row below the last detail.

```text
WARN  unrestricted access enabled; authenticated users bypass role restrictions.
WARN  no TLS on the listener; plaintext passwords allowed from loopback clients.
WARN  skipped 3 unreadable Config resources
      11111111-1111-1111-1111-111111111111.7  invalid UTF-8
      22222222-2222-2222-2222-222222222222.7  invalid UTF-8
      33333333-3333-3333-3333-333333333333.7  invalid brace serialization
metadata  Config  [==============>-]   95%  118/120 res  15.2 MiB/16.0 MiB
```

Successful completion replaces only the live row with a permanent
summary. The listen line follows after binding.

```text
metadata  complete  120 resources  16.0 MiB  1.4 s
INFO  Serving on 127.0.0.1:5433 (no TLS)
```

Register shutdown handling before startup acquisition. If Ctrl+C arrives
while a redraw or warning is pending, finish the serialized line operation
and prioritize shutdown before another repaint. Flush any pending
warnings, remove the live row, and print one newline-terminated
`metadata  interrupted` summary with last known counts and elapsed time.
Close or drop the abandoned session through its normal lifecycle. After
the existing connection shutdown path, print `INFO  Serve stopped`. This
graceful exit returns 0 during startup, refresh, and serving. A failed
refresh prints `ERROR` and continues with the last good snapshot.

### Help, argument errors, and execution errors

Recognize whole help tokens in a known command scope before config reading
or startup validation. Select top-level, command, or provider help from the
recognized prefix. Resolve known option names before consuming their
values. Use the same short usage description in the scoped help and
argument error. Keep no-argument help and the `repl` alias. Keep exit 2 for
argument errors and retain the existing nonzero execution status mapping
(usually 1 for lexical, metadata, or data errors, 2 for I/O and driver
errors). Keep successful handling for a closed stdout consumer. The
following outcomes apply to the cases shown.

| Invocation or event | stdout | stderr | Status |
| --- | --- | --- | --- |
| `open-sdbl metadata --help` or `open-sdbl serve --help` | Scoped help | Empty | 0 |
| `open-sdbl metadata postgres --host db` | Empty | Missing options, short usage and scoped help | 2 |
| `open-sdbl serve postgres --wat` | Empty | Unknown option, short usage and scoped help | 2 |
| `open-sdbl metadata --config /nonexistent/open-sdbl-terminal-ux.toml` | Empty | Config read operation, path and I/O cause | 2 |
| Gateway startup fails loading the missing certificate above | Empty | TLS setup, path and I/O cause | 2 |
| Gateway startup reads an empty certificate file | Empty | TLS setup, path and missing certificate | 1 |
| PostgreSQL DBNames read encounters the missing relation | Empty | Operation and safe server cause, including `42P01` | 2 |
| One authenticated gateway client cannot open a backing session | Empty | Permanent session error with peer and cause | Process stays running |

Example argument diagnostic for `metadata`.

```text
ERROR  Missing required options --database and --user.
Usage  open-sdbl metadata postgres --host HOST --database DB --user USER [options]
Help   open-sdbl metadata postgres --help
```

Example argument diagnostic and separate runtime diagnostic for `serve`.

```text
ERROR  Unknown option --wat for serve postgres.
Usage  open-sdbl serve postgres --host HOST --database DB --user USER [options]
Help   open-sdbl serve postgres --help

ERROR  Cannot read the gateway TLS certificate.
Cause  File not found at "/nonexistent/open-sdbl-terminal-ux.crt".
Next   Set --tls-cert to a readable PEM certificate chain.
```

The driver version locked by the workspace, `tokio-postgres 0.7.18`,
renders its database error kind as `db error` in `src/error/mod.rs:380`.
The original cause is accessible through `Error::as_db_error()` and
`Error::source()`. The server record has `code()`, `message()`, and
`severity()`, with optional detail, hint, position, schema, table, column,
constraint, and other fields. Relation information can be present only
in the primary message. `open-sdbl-db/src/error.rs:187` formats the driver
into `DbError::Database(String)`, whose `source()` returns `None`. At
`db/postgres/metadata.rs:595`, the label for the DBNames query goes to the
timeout wrapper, which uses it only on timeout. The error conversion drops
the server fields and the ordinary query error loses the operation label.
The analogous connection conversion is at `open-sdbl-db/src/error.rs:59`.

Carry a typed PostgreSQL source and the existing operation label across
the acquisition and query boundaries. Provide a safe diagnostic view
to the CLI and gateway event adapter. Check SQLSTATE and use known query
context or approved driver fields for a short cause. The fixture's trusted
DBNames operation and SQLSTATE allow the fixed relation name below. A
localized or unexpected primary message can still be rendered as a safe
SQLSTATE description. Connection failures without a server record use a
recognizable I/O or TLS category when available. Inspect the typed TLS
cause inside the I/O error. An unknown issuer points to `--trust-ca-file`
for a private CA. A hostname mismatch points to `--host` and the name on
the server certificate. Render dynamic fields through the existing escape
helper. Avoid raw source-chain or debug dumps,
server DETAIL/HINT/context, connection strings, SQL literals, credentials,
and TOML lines that may carry a secret. Generate next actions for known
cases. Keep the legacy plaintext opt-in code and its stable plain-text
fields.

```text
metadata  failed  46 ms
ERROR  PostgreSQL could not read DBNames.
Cause  Relation "params" does not exist (SQLSTATE 42P01).
Next   Check that --database identifies a 1C information base.
```

The same safe view serves recoverable query errors in
`open-sdbl-cli/src/repl/mod.rs:656`, where phase timing and rollback remain
available. For a failed post-authentication backing connection,
`open-sdbl-pgwire/src/connection.rs:413` already logs an event and returns
to close that session. Give that event the peer and safe cause before the
error is flattened for display. The client gets the existing `08006`
response. Keep normal wire diagnostics brief and keep existing
`--verbose-errors` opt-in for safe detail. The separate deliberate
`--debug-authentication-secrets` behavior remains as specified.

```text
ERROR  Cannot open a PostgreSQL backing session for 127.0.0.1:55102.
Cause  Connection refused by the backing database endpoint.
```

### Backing transport and listener TLS

`args.rs:23` populates `PostgresConnection` or `MsSqlConnection` in
`open-sdbl-db/src/connection.rs`. Console and metadata reach
`DatabaseSession::connect` from `app.rs`. Gateway startup, inspection, and
refresh go through `open-sdbl-pgwire/src/state.rs:69`. Client sessions and
reopens use `DbEngine` in `open-sdbl-pgwire/src/engine.rs:99`. PostgreSQL
uses `SslMode::Disable` with `NoTls`, or `SslMode::Require` plus the
Rustls verifier for the requested policy, both directly and over SOCKS5.
The mode also applies to query cancellation. MSSQL constructs
`tiberius::Config` once for direct or proxied connections. It currently
leaves encryption unset, and the pinned Tiberius `native-tls` build
defaults to `EncryptionLevel::Required` with system trust and hostname
verification. MSSQL cancellation reconnects with the stored setup.

| Current flag or setting | Applies to | Observed in code and default | Canonical syntax | Compatibility |
| --- | --- | --- | --- | --- |
| `--sslmode MODE`, `sslmode`, `PGSSLMODE` | PostgreSQL backing sessions | Explicit flag or TOML wins over env. Default `verify-full` requires TLS and checks chain and name. `verify-ca` checks chain. `require` uses TLS without peer verification. `disable` uses NoTls with the explicit legacy opt-in. No plaintext fallback. | `--tls-mode MODE` or `tls_mode = "MODE"` | Keep PostgreSQL legacy modes and env precedence. Keep historical last-value behavior for repeated legacy `--sslmode`. |
| `--insecure-plaintext`, `insecure_plaintext` | PostgreSQL backing sessions | False by default. Valid only with effective legacy `disable`. The flag authorizes plaintext and does not choose the mode on its own. | `--tls-mode disable` | Keep the legacy pair including env plus opt-in. Preserve the opt-in diagnostic for a missing half of the pair. |
| No MSSQL plaintext flag | MSSQL backing sessions | CLI always uses Tiberius `Required` with this build. No current CLI or TOML spelling for plaintext. | `--tls-mode disable` | Newly supported explicit mode. Verified TLS remains the default. |
| `--trust-server-certificate`, `trust_server_certificate` | MSSQL backing sessions | False by default. Tiberius `trust_cert()` keeps encryption and skips chain and hostname checks. | `--tls-mode require` | Keep MSSQL-only alias and permanent warning. |
| `--trust-ca-file`, `trust_ca_file` | Both backing drivers | Absent by default. PostgreSQL uses the file instead of native roots in verifying modes. The MSSQL native TLS driver adds the PEM/CRT/DER certificate to its trust store and still verifies the name. | `--trust-ca-file PATH` with `verify-full`, also `verify-ca` for PostgreSQL | Keep flag, key, provider-specific trust semantics and validation. |
| `--tls-cert`, `--tls-key`, matching TOML keys | Incoming `serve` | Absent by default. Pair configures Rustls server mode and advertises TLS on SSLRequest. It authenticates the server, with no client certificate requirement. | `--tls-cert PATH --tls-key PATH` | Keep pairing, PEM files, and listener validation. |
| `--allow-insecure-loopback`, `allow_insecure_loopback` | Incoming `serve` | False by default. Allows loopback clients to submit a password without TLS. Default listener is `127.0.0.1:5433`. Without listener TLS or opt-in, clients are refused before password entry. | Same flag or key under listener options | Keep as an independent opt-in and log the policy. |
| `--allow-insecure-network`, `allow_insecure_network` | Incoming `serve` | False by default. Allows plaintext password exchange from any peer and permits non-loopback bind without TLS. Can coexist with listener TLS. Includes loopback when both opts are present. | Same flag or key under listener options | Keep old guard and make the policy warning permanent. |

The pinned MSSQL path is `open-sdbl-db/src/db/mssql/session.rs:191` and
Tiberius 0.12.3 `src/client/config.rs`, `src/client/connection.rs`, and
`src/client/tls_stream/native_tls_stream.rs`. For new explicit plaintext,
pass `EncryptionLevel::NotSupported`. `Off` still encrypts login.
Tiberius's `tds/codec/pre_login.rs::negotiated_encryption` can choose TLS
if the peer answers with a different mode even when the client sends
`NotSupported`. Gate the initial server PRELOGIN response on the same
bounded stream before Tiberius can proceed to LOGIN7. Forward an unchanged
valid `NotSupported` reply to the driver. Stop with a specific policy error
if the server requires or selects encryption, and surface malformed or
missing prelogin data as a transport failure. This small check reuses the
existing transport, timeout, and TLS driver. It applies after either TCP or
SOCKS5 setup and to MSSQL reconnects.

Use one normalized TLS selection for startup, gateway per-client sessions,
reload, inspection, and reconnect. Pass the MSSQL plaintext policy via
additive database and gateway connection entry points whose existing
entry points default to the current policy. Preserve existing public
connection and gateway struct literals. Keep an explicit MSSQL
`verify-full` on the existing verified setup and map `require` to
`Required` with the existing trust override. Reject MSSQL `verify-ca`
before opening a socket. For PostgreSQL, apply the normalized mode in
`open-sdbl-db/src/db/postgres/session.rs:99` and `:210`.

Canonical selection overrides `PGSSLMODE`. Mixing `--tls-mode` with an
active legacy mode or opt-in in the same invocation or TOML file is an
argument error, including equivalent selections. Identical repeated
canonical choices are accepted and different canonical choices are
rejected. Keep the legacy-only order and validation rules, including
rejecting `--trust-server-certificate` on PostgreSQL, `--sslmode` on
MSSQL, and `--insecure-plaintext` outside its legacy PostgreSQL case. A
false legacy TOML Boolean supplies no flag. `--trust-ca-file` conflicts
with either unverified mode. Security warnings occur after successful
argument validation and before the first connection.

PostgreSQL today and after, using the same credentials and plaintext-capable
database.

```console
$ open-sdbl console postgres --host db --database unf --user reader --sslmode disable --insecure-plaintext
$ open-sdbl console postgres --host db --database unf --user reader --tls-mode disable
```

MSSQL today and after, showing the current rejected attempt and the new
plaintext invocation for a server that permits it.

```console
$ open-sdbl console mssql --host sql --database unf --user reader --sslmode disable
unknown console option "--sslmode"
$ open-sdbl console mssql --host sql --database unf --user reader --tls-mode disable
```

The first MSSQL invocation exits 2. The accepted current option
`--trust-server-certificate` maps to `--tls-mode require` and keeps
encryption with peer verification disabled.

The loader in `serve.rs:178` supplies the listener certificate and key to
`open-sdbl-pgwire/src/lib.rs:328`, with negotiation and password gating
in `connection.rs:238` and `:332`. Those settings stay independent of the
normalized backing mode. Also log a successful plaintext admission with
peer and user, using the negotiated session state.

In scoped help use `Connection` for host, database, user, port, and config,
and `Backing TLS and proxy` for mode, CA, and SOCKS options. List each shared
option once. Show provider defaults and `--mssql-dialect` where applicable.
Use `Listener` for bind, server certificate and key, and insecure-client
rules. Use `Advanced` for `--catalog-memory-limit-mib`,
`--session-param`, `--verbose-errors`, `--disable-rls-check`, and
authentication debug options. Keep documented the bypass scope and the
explicit sensitive-authentication behavior. Move legacy TLS forms, the
`repl` alias, and provider environment details into the scoped
`Compatibility` section or README. Keep `--config PATH` exclusive of
runtime options and type-check gateway-only TOML fields reused by
console or metadata.

## Risks / Trade-offs

- Resize or cancellation can race a redraw. Track the visible rows owned
  by the scene and give cleanup precedence over repaint. Test the boundaries
  with a PTY, including input queued before and during handoff.
- Driver messages can copy secrets or SQL values. Use fixed, contextual
  wording and an explicit safe field set. Exercise custom ErrorResponse
  text with synthetic secrets and terminal controls.
- The meaning of plaintext varies across TDS drivers. Verify the actual
  negotiated transport for a plaintext-capable SQL Server and for one
  requiring encryption, and assert no login on a rejected PRELOGIN.
- Scripts may scrape the former help or long stderr messages. Keep exit
  statuses, machine-oriented stdout, and the stable plaintext opt-in
  code, and document the intentional human-output changes.

## Migration Plan

Keep valid legacy CLI and TOML spellings accepted throughout this change.
Update README connection examples and deployment documentation with
`--tls-mode` and `tls_mode` when they select a mode. Explain that
`--tls-cert` and `--tls-key` still protect incoming connections and that
omitting outgoing options retains verified defaults. Keep an example of
the accepted legacy PostgreSQL pair for older installations. Existing
config files and flags can be kept for rollback to older binaries. An MSSQL
plaintext invocation requires the new version.
