#!/usr/bin/env python3
"""Пересчитывает долю компилируемых запросов фикстуры и закрепляет её.

    pin_share.py tests/fixtures/unf

Обновляет строку «Компилируется N запросов из M» в README фикстуры и
запись `"<имя>" => Some((N, M))` в `tests/query_corpus.rs`.
"""

import collections
import json
import os
import re
import sys


def main() -> None:
    root = sys.argv[1].rstrip("/")
    name = os.path.basename(root)
    corpus = [json.loads(line) for line in open(f"{root}/corpus.jsonl", encoding="utf-8") if line.strip()]
    expected = [json.loads(line) for line in open(f"{root}/expected.jsonl", encoding="utf-8") if line.strip()]
    assert len(corpus) == len(expected)
    beyond = sum(1 for entry in corpus if entry.get("expect") == "fixture")
    compiled = sum(1 for outcome in expected if not outcome.startswith("!"))
    total = len(corpus) - beyond
    readme = f"{root}/README.md"
    text = open(readme, encoding="utf-8").read()
    text, n = re.subn(r"Компилируется \d+ запросов из \d+", f"Компилируется {compiled} запросов из {total}", text)
    # The table of refusal causes follows the header row; it is rebuilt
    # from the recorded diagnostics, identifiers masked.
    causes = collections.Counter()
    for entry, outcome in zip(corpus, expected):
        if outcome.startswith("!") and entry.get("expect") != "fixture":
            message = outcome[1:].split(": ", 1)[-1]
            causes[re.sub(r'"[^"]*"', '"…"', message)[:90]] += 1
    header = "| Причина | Запросов |\n| --- | --- |\n"
    if header in text:
        start = text.index(header) + len(header)
        end = text.find("\n\n", start)
        rows = "".join(f"| {cause} | {count} |\n" for cause, count in causes.most_common(12))
        text = text[:start] + rows.rstrip("\n") + text[end:]
    open(readme, "w", encoding="utf-8").write(text)
    test = "tests/query_corpus.rs"
    source = open(test, encoding="utf-8").read()
    source, m = re.subn(rf'"{name}" => Some\(\(\d+, \d+\)\)', f'"{name}" => Some(({compiled}, {total}))', source)
    open(test, "w", encoding="utf-8").write(source)
    print(f"{name}: {compiled} of {total} (README {'updated' if n else 'untouched'}, test {'updated' if m else 'untouched'})")


if __name__ == "__main__":
    main()
