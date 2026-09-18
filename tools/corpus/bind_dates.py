#!/usr/bin/env python3
"""Привязывает даты к параметрам, которых требует эталон.

Читает `expected.jsonl` фикстуры, и для каждой записи с диагностикой
«parameter "&X" must be a date» дописывает в `corpus.jsonl` значение
`D20240101` (или `D20241231235959`, если имя похоже на конец периода).
Запускать чередуя с перезаписью эталона, пока не перестанет находить:

    while [ "$(bind_dates.py tests/fixtures/unf)" != "bound 0" ]; do
      CORPUS_FIXTURE=unf cargo test -p open-sdbl --test query_corpus -- --ignored rerecord
    done
"""

import json
import re
import sys

NEEDS_DATE = re.compile(r'^!Parameter: parameter "&([^"]+)" must be a date')


def main() -> None:
    root = sys.argv[1]
    corpus = [line.rstrip("\n") for line in open(f"{root}/corpus.jsonl", encoding="utf-8") if line.strip()]
    expected = [json.loads(line) for line in open(f"{root}/expected.jsonl", encoding="utf-8") if line.strip()]
    assert len(corpus) == len(expected), "corpus and expected are not aligned"
    bound = 0
    out = []
    for line, outcome in zip(corpus, expected):
        match = NEEDS_DATE.match(outcome)
        if match:
            entry = json.loads(line)
            name = match.group(1)
            low = name.lower()
            end = any(word in low for word in ("конец", "окончани", "end")) or low.startswith("по")
            entry.setdefault("params", {})[name] = "D20241231235959" if end else "D20240101"
            line = json.dumps(entry, ensure_ascii=False)
            bound += 1
        out.append(line)
    with open(f"{root}/corpus.jsonl", "w", encoding="utf-8") as f:
        f.write("\n".join(out) + "\n")
    print(f"bound {bound}")


if __name__ == "__main__":
    main()
