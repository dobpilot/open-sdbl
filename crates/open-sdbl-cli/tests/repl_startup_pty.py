"""Drive private initialization gates in the Rust test binary on a real PTY."""
import os
import fcntl
import struct
import re
import select
import subprocess
import sys
import termios
import time

from terminal_ux import controlling_terminal, read_until, stop, terminal


def screen_lines(transcript):
    """Interpret the normal-screen operations emitted by the startup region."""
    lines = [[]]
    row = column = 0
    for token in re.findall(r"\x1b\[[?\d;]*[A-Za-z~]|.", transcript.decode(), re.S):
        if token == "\x1b[A":
            row = max(0, row - 1)
        elif token == "\x1b[2K":
            lines[row].clear()
        elif token.startswith("\x1b"):
            assert token in ("\x1b[?25l", "\x1b[?25h", "\x1b[?2004l", "\x1b[?2004h"), token
        elif token == "\r":
            column = 0
        elif token == "\n":
            row += 1
            if row == len(lines):
                lines.append([])
        else:
            line = lines[row]
            if column >= len(line):
                line.extend(" " * (column + 1 - len(line)))
            line[column] = token
            column += 1
    return ["".join(line) for line in lines]


def resize_while_loading(master, slave):
    trace = b""
    status = b"open-sdbl  Config  31/48 res  12.0/24.0 MiB"
    for rows, cols, art_rows in [(40, 120, 29), (40, 64, 15), (24, 50, 0), (24, 80, 19)]:
        fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
        received = b""
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            chunk = read_until(master, status)
            received += chunk
            trace += chunk
            # A queued paint may still use the previous viewport.
            paints = received.split(b"\x1b[?25l")[1:]
            if any(status in paint and
                   paint.split(status, 1)[0].count(b"\n") == art_rows
                   for paint in paints):
                break
        else:
            raise AssertionError(f"no {cols}×{art_rows} paint after resize")
    return trace


def check(binary, motif, outcome, static=False):
    master, slave = terminal(80)
    read_fd, write_fd = os.pipe()
    original = termios.tcgetattr(slave)
    env = dict(os.environ, TERM="dumb" if static else "xterm-256color",
               NO_COLOR="1", LC_ALL="C.UTF-8", SDBL_TEST_STARTUP_PIPE=str(read_fd),
               SDBL_TEST_STARTUP_MOTIF=str(motif))
    env.pop("CI", None)
    command = [binary, "--exact", "repl::startup::tests::startup_boundaries_in_a_pty",
               "--nocapture"]
    if outcome == "fast":
        os.write(write_fd, b"ready\n")
    try:
        proc = subprocess.Popen(command, stdin=slave, stdout=slave, stderr=slave, env=env,
                                pass_fds=(read_fd,), preexec_fn=controlling_terminal)
        try:
            if outcome == "fast":
                before = read_until(master, b"open-sdbl=> ")
                assert b"\x1b[?25l" not in before
                os.write(master, "ВЫБРАТЬ 1;\r".encode())
                assert proc.wait(timeout=5) == 0
            else:
                trace = read_until(master, b"open-sdbl  connecting")
                if not static:
                    flags = termios.tcgetattr(slave)[3]
                    assert flags & termios.ISIG and flags & termios.NOFLSH
                for gate, stage in [(b"Config", b"Config  31/48 res"),
                                    (b"warn", b"WARN  skipped 1 unreadable Config resources"),
                                    (b"commit", b"open-sdbl  commit"),
                                    (b"editor", b"open-sdbl  line editor")]:
                    if outcome == "connecting":
                        break
                    os.write(write_fd, gate + b"\n")
                    # Static display reports totals at entry and on departure;
                    # a stalled batch is intentionally silent.
                    if static and gate == b"Config":
                        stage = b"Config   0/48 res"
                    trace += read_until(master, stage)
                    if gate == b"Config" and outcome == "resize":
                        trace += resize_while_loading(master, slave)
                    if gate.decode() == outcome:
                        break
                assert b"open-sdbl=>" not in trace
                if outcome == "interrupt":
                    os.write(master, b"\x03")
                    diag = read_until(master, b"Cancelled\r\n")
                    assert proc.wait(timeout=5) == 130
                else:
                    os.write(write_fd, b"fail\n")
                    diag = read_until(master, b"fixture cause\r\n")
                    assert proc.wait(timeout=5) == 2
                while select.select([master], [], [], 0)[0]:
                    diag += os.read(master, 8192)
                screen = screen_lines(trace + diag)
                assert "running 1 test" in screen
                if outcome in ("editor", "resize", "interrupt"):
                    line = screen.index("WARN  skipped 1 unreadable Config resources")
                    assert screen[line+1] == "      resource.7  invalid UTF-8"
                assert not any(re.search("[⠀-⣿]", line) for line in screen)
                assert b"open-sdbl=>" not in diag
                if static:
                    assert b"\x1b" not in trace + diag
                else:
                    assert b"\x1b[?25h" in diag
                    assert b"\x1b[?25l" not in diag[diag.rfind(b"\x1b[?25h"):]
                if outcome in ("editor", "resize", "interrupt"):
                    assert re.search(rb"WARN  skipped 1 unreadable Config resources\r\n"
                                     rb"      resource\.7  invalid UTF-8\r\n", trace)
            assert termios.tcgetattr(slave) == original
        finally:
            stop(proc)
    finally:
        for fd in [read_fd, write_fd, master, slave]:
            os.close(fd)


if __name__ == "__main__":
    for index in range(3):
        check(sys.argv[1], index, "resize")
    for stage in ("fast", "connecting", "Config", "interrupt"):
        check(sys.argv[1], 0, stage)
    check(sys.argv[1], 1, "editor", static=True)
