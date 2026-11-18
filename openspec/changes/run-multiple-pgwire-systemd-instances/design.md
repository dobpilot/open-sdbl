# Design

Ship `deploy/open-sdbl-pgwire@.service` alongside the existing unit. Systemd
expands `%i` in the template to select
`/etc/open-sdbl/instances/%i.toml` and the optional
`/etc/open-sdbl/instances/%i.env`. The unit runs
`/usr/local/bin/open-sdbl serve --config ...` as the existing dedicated
`open-sdbl` user and group, with the existing restart and hardening settings.
Each invocation is its own process: its configuration supplies one listener,
one backend and its own TLS, session and authorization settings. No registry
or new CLI option is needed. The operator must choose non-conflicting listen
addresses and ports; the OS reports a bind conflict to the affected process.

The environment file uses systemd's `EnvironmentFile` syntax, not shell
syntax. It can point `PGPASSFILE` at an instance-specific pgpass file or
provide the existing MSSQL and SOCKS5 environment variables. Documentation
must explain that environment variables are not a secret store, use restricted
permissions, and give the service account read access to its required files.
Passwords remain outside TOML and command-line arguments. A shared Unix service
account isolates configuration and process state operationally, but does not
provide a security boundary between instances: that account can read every file
granted to it. Operators requiring OS-level isolation need separate accounts
and unit overrides.

Document simple instance names such as `sales` and `warehouse` and systemd's
escaping rules for other names; filenames correspond to the unit's escaped
instance identifier. Configuration is read at process startup: restart the
affected instance after changing TOML or its environment. SIGHUP refreshes
metadata, not the TOML or environment. Journal entries and lifecycle commands
are addressed to the individual unit. A failed or stopped instance must not
stop another; systemd's restart policy may retry a failed instance.

The existing `open-sdbl-pgwire.service`, paths, and instructions remain usable.
No automatic migration, global enablement, configuration generation or new
credential storage mechanism is introduced.
