#!/usr/bin/env python3
"""Extracts the statements of a PostgreSQL server log into corpus files.

The recording server runs with `log_statement = all` and
`log_line_prefix = '%a|'`, so every statement line reads
`<application_name>|LOG:  statement: <text>` and its continuation lines
start with a tab. Statements are grouped by application name (the client
tag), deduplicated in first-seen order, and written as one file per tag:
`<tag>.sql`, statements separated by a line holding only `-- next`.

Run: python3 -I extract.py SERVER_LOG OUTPUT_DIR [TAG...]
"""

import os
import re
import sys

LINE = re.compile(r"^(?P<tag>[^|]*)\|(?:LOG|ЖУРНАЛ):\s+(?:statement|execute [^:]*|оператор|выполнение [^:]*):\s?(?P<text>.*)$")


def statements(lines):
    current = None
    for line in lines:
        line = line.rstrip("\n")
        match = LINE.match(line)
        if match:
            if current:
                yield current
            current = [match["tag"], match["text"]]
        elif line.startswith("\t") and current:
            current[1] += "\n" + line[1:]
        else:
            if current:
                yield current
            current = None
    if current:
        yield current


def main():
    log, output, *tags = sys.argv[1:]
    grouped = {}
    with open(log, encoding="utf-8", errors="replace") as handle:
        for tag, text in statements(handle):
            if tags and tag not in tags:
                continue
            seen = grouped.setdefault(tag, [])
            text = text.strip()
            if text and text not in seen:
                seen.append(text)
    os.makedirs(output, exist_ok=True)
    for tag, texts in grouped.items():
        with open(os.path.join(output, f"{tag}.sql"), "w", encoding="utf-8") as handle:
            handle.write("\n-- next\n".join(texts) + "\n")
        print(f"{tag}: {len(texts)} statements")


if __name__ == "__main__":
    main()
