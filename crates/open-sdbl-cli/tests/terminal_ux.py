"""Run with a disposable database seeded by fixtures/terminal-ux/<provider>.sql.

Example:
    python3 crates/open-sdbl-cli/tests/terminal_ux.py target/debug/open-sdbl \
        --host 127.0.0.1 --port 39172 --database ux_golden --user ux_review

The two fixtures produce the same f5621ca stdout. Capture the PostgreSQL
baseline with --sslmode disable --insecure-plaintext and the MSSQL baseline
with its verified defaults or --trust-server-certificate. All captures are
binary. Set MSSQL_PASSWORD in the environment for an MSSQL run.
"""
import argparse
import errno
import fcntl
import os
from pathlib import Path
import pty
import re
import select
import struct
import socket
import socketserver
import threading
import subprocess
import tempfile
import termios
import time


def terminal(columns=100):
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, columns, 0, 0))
    return master, slave


def read_until(master, needle, captured=b""):
    deadline = time.monotonic() + 20
    while needle not in captured:
        remaining = deadline - time.monotonic()
        if remaining <= 0 or not select.select([master], [], [], remaining)[0]:
            raise AssertionError(f"{needle!r} missing, tail {captured[-700:]!r}")
        try:
            captured += os.read(master, 8192)
        except OSError as error:
            if error.errno != errno.EIO:
                raise
            raise AssertionError(f"PTY closed before {needle!r}: {captured!r}") from error
    return captured


def controlling_terminal():
    os.setsid()
    fcntl.ioctl(0, termios.TIOCSCTTY, 0)


def stop(process):
    if process.poll() is None:
        process.kill()
        process.wait()


def check_metadata(command, environment, reference):
    with tempfile.TemporaryDirectory() as directory:
        directory = Path(directory)
        pty_snapshot = directory / "pty.stdout"
        master, slave = terminal()
        try:
            with pty_snapshot.open("wb") as stdout:
                process = subprocess.Popen(command, stdout=stdout, stderr=slave, env=environment)
                try:
                    displayed = read_until(master, b"metadata  complete  5 resources  156 B")
                    assert b"\x1b[38;2;181;245;220mme\x1b[0m" in displayed
                    warning = b"\r\x1b[2K\x1b[1;33mWARN\x1b[0m  skipped 4 unreadable Config resources\r\n"
                    before, after = displayed.split(warning, maxsplit=1)
                    assert b"0%  0/5 resources    0 B/156 B" in before
                    for resource in range(1, 5):
                        line = f"      00000000-0000-0000-0000-{resource:012d}.7  invalid UTF-8\r\n"
                        assert after.startswith(line.encode())
                        after = after[len(line):]
                    assert b"100%  5/5 resources  156 B/156 B" in after
                    assert b"MiBwarning:" not in displayed
                    summary = read_until(master, b"\r\n", displayed[displayed.rfind(b"metadata  complete"):])
                    assert re.match(rb"metadata  complete  5 resources  156 B  [\d.]+ (ms|s)\r\n", summary)
                    assert process.wait(timeout=5) == 0
                finally:
                    stop(process)
        finally:
            os.close(master)
            os.close(slave)
        file_snapshot = directory / "file.stdout"
        stderr_log = directory / "stderr.log"
        with file_snapshot.open("wb") as stdout, stderr_log.open("wb") as stderr:
            subprocess.run(command, stdout=stdout, stderr=stderr, env=environment, check=True)
        assert pty_snapshot.read_bytes() == reference
        assert file_snapshot.read_bytes() == reference
        diagnostics = stderr_log.read_bytes()
        assert b"\x1b" not in diagnostics and b"\r" not in diagnostics
        assert diagnostics.count(b"invalid UTF-8") == 4


class GatedConnection(socketserver.BaseRequestHandler):
    """Hold the real connection until the PTY driver releases it."""
    def handle(self):
        if not self.server.release.wait(15):
            return
        try:
            with socket.create_connection(self.server.destination, timeout=5) as upstream:
                upstream.settimeout(None)
                peers = (self.request, upstream)
                while ready := select.select(peers, [], [], 15)[0]:
                    for reader in ready:
                        data = reader.recv(65536)
                        if not data:
                            return
                        (upstream if reader is self.request else self.request).sendall(data)
        except ConnectionError:
            return


def check_paste(command, environment, destination, scenario):
    master, slave = terminal(80)
    environment = dict(environment, NO_COLOR="1", LC_ALL="C.UTF-8")
    with socketserver.ThreadingTCPServer(("127.0.0.1", 0), GatedConnection) as gate:
        gate.daemon_threads = True
        gate.destination = destination
        gate.release = threading.Event()
        threading.Thread(target=gate.handle_request, daemon=True).start()
        command = command.copy()
        command[command.index("--host") + 1] = "127.0.0.1"
        command[command.index("--port") + 1] = str(gate.server_address[1])
        try:
            original = termios.tcgetattr(slave)
            process = subprocess.Popen(command, stdin=slave, stdout=slave, stderr=slave,
                                       env=environment, preexec_fn=controlling_terminal)
            try:
                before = read_until(master, b"open-sdbl  connecting")
                assert b"open-sdbl=> " not in before and b"\x1b[?25l" in before
                assert b"\x1b[32m" not in before
                if scenario == "interrupt":
                    os.write(master, b"\x03")
                    done = read_until(master, b"Cancelled\r\n")
                    assert b"open-sdbl=> " not in done and b"\x1b[?25h" in done
                    assert process.wait(timeout=5) == 130
                else:
                    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 8, 20, 0, 0))
                    narrow = read_until(master, b"\x1b[?25lconnecting")
                    assert b"open-sdbl=> " not in narrow
                    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
                    expanded = read_until(master, b"open-sdbl  connecting")
                    assert expanded.count(b"\n") >= 18
                    query = "ВЫБРАТЬ 1;".encode()
                    # Split inside the first UTF-8 character and finish the
                    # bracketed paste only after rustyline draws its prompt.
                    prefix = query[:1] if scenario == "split" else query + b"\x1b[201~"
                    os.write(master, b"\x1b[200~" + prefix)
                    gate.release.set()
                    prompt = read_until(master, b"open-sdbl=> ")
                    assert prompt.count(b"\\help for commands") == 1
                    assert b"skipped 4 unreadable Config resources\r\n" in prompt
                    if scenario == "split":
                        os.write(master, query[1:] + b"\x1b[201~")
                    os.write(master, b"\r")
                    result = read_until(master, b"1 row")
                    assert result.count(b"1 row") == 1
                    # Up recalls exactly the first line. Tab completes the
                    # real meta-command from the installed console helper.
                    os.write(master, b"\x1b[A\r")
                    read_until(master, b"1 row")
                    os.write(master, b"\\hel\t\r")
                    read_until(master, b"show this help")
                    os.write(master, b"\\q\r")
                    assert process.wait(timeout=5) == 0
                assert termios.tcgetattr(slave) == original
            finally:
                stop(process)
                gate.release.set()
        finally:
            os.close(master)
            os.close(slave)


def check_console_streams(command, environment):
    # Keep stderr on a terminal while independently capturing scripted stdout.
    master, slave = terminal()
    try:
        batch = subprocess.run(command, input=b"\\q\n", stdout=subprocess.PIPE, stderr=slave,
                               env=environment, timeout=15, check=True)
        assert not batch.stdout
        static = read_until(master, b"skipped 4 unreadable")
        assert b"\x1b" not in static
        failed = subprocess.run([command[0], "console", "postgres", "--invalid"],
                                input=b"", stdout=subprocess.PIPE, stderr=slave,
                                env=environment, timeout=5)
        assert failed.returncode == 2 and not failed.stdout
        assert b"\x1b" not in read_until(master, b"--help")
    finally:
        os.close(master)
        os.close(slave)

    for mode in ("static", "file", "ci"):
        master, slave = terminal()
        with tempfile.TemporaryFile() as log:
            try:
                env = dict(environment)
                if mode == "static":
                    env["TERM"] = "dumb"
                if mode == "ci":
                    env["CI"] = "true"
                proc = subprocess.Popen(command, stdin=slave, stdout=slave,
                                        stderr=log if mode == "file" else slave,
                                        env=env, preexec_fn=controlling_terminal)
                try:
                    if mode == "ci":
                        os.write(master, b"\\q\n")
                        data = read_until(master, b"skipped 4 unreadable")
                        assert b"\x1b" not in data and b"open-sdbl=>" not in data
                    else:
                        data = read_until(master, b"\\help for commands")
                        if mode == "static":
                            assert b"\x1b" not in data
                            for phase in [b"connecting", b"transaction", b"layout", b"DBNames",
                                          b"Config", b"extensions", b"SchemaStorage", b"catalog",
                                          b"resolve", b"commit", b"line editor"]:
                                pos = data.index(b"open-sdbl  " + phase)
                                data = data[pos + len(b"open-sdbl  " + phase):]
                        os.write(master, b"\\q\r")
                    assert proc.wait(timeout=5) == 0
                    if mode == "file":
                        log.seek(0)
                        diagnostics = log.read()
                        assert b"skipped 4 unreadable" in diagnostics and b"\x1b" not in diagnostics
                finally:
                    stop(proc)
            finally:
                os.close(master)
                os.close(slave)


def check_help(binary, environment):
    for overrides, color in [
        ({"COLORFGBG": "15;0"}, b"\x1b[38;2;52;213;154m"),
        ({"COLORFGBG": "0;15"}, b"\x1b[38;2;8;117;78m"),
        ({"COLORTERM": ""}, b"\x1b[32m"),
        ({"NO_COLOR": "1"}, None),
        ({"CI": "true"}, None),
        ({"TERM": "dumb"}, None),
    ]:
        master, slave = terminal(32)
        try:
            process = subprocess.Popen([binary, "--help"], stdout=slave, stderr=subprocess.PIPE,
                                       env=dict(environment, **overrides))
            try:
                displayed = read_until(master, b"Run open-sdbl <command> --help")
                assert b"Example" in displayed and displayed.count(b"Example") == 1
                assert b"\x1b[2K" not in displayed
                if color is None:
                    assert b"\x1b" not in displayed
                else:
                    assert color in displayed
                assert process.wait(timeout=5) == 0 and not process.stderr.read()
            finally:
                stop(process)
        finally:
            os.close(master)
            os.close(slave)
    output = subprocess.run([binary, "--help"], env=environment, capture_output=True, check=True)
    assert b"\x1b" not in output.stdout and not output.stderr


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary")
    parser.add_argument("--provider", choices=("postgres", "mssql"), default="postgres")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port")
    parser.add_argument("--database")
    parser.add_argument("--user", default="ux_review")
    parser.add_argument("--tls-mode", choices=("disable", "require", "verify-full"), default="disable")
    parser.add_argument("--reference", type=Path)
    args = parser.parse_args()
    if args.provider == "mssql" and not args.database:
        parser.error("the MSSQL fixture requires --database")
    port = args.port or ("5432" if args.provider == "postgres" else "1433")
    options = [args.provider, "--host", args.host, "--port", port,
               "--database", args.database or "postgres", "--user", args.user,
               "--tls-mode", args.tls_mode]
    environment = dict(os.environ, TERM="xterm-256color", COLORTERM="truecolor", COLORFGBG="15;0")
    for key in ("CI", "NO_COLOR", "PGSSLMODE"):
        environment.pop(key, None)
    reference = args.reference or Path(__file__).with_name("fixtures") / "terminal-ux" / "f5621ca.tsv"
    check_metadata([args.binary, "metadata", *options], environment, reference.read_bytes())
    command = [args.binary, "console", *options]
    for scenario in ("full", "split", "interrupt"):
        check_paste(command, environment, (args.host, int(port)), scenario)
    check_console_streams(command, environment)
    check_help(args.binary, environment)


if __name__ == "__main__":
    main()
