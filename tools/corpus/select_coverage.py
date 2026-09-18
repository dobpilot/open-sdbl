#!/usr/bin/env python3
"""Отбирает из полного корпуса подмножество, покрывающее регистры.

Для регистров накопления берёт не больше `--cap` самых коротких запросов
на каждую пару «регистр, виртуальная таблица» (главная таблица считается
отдельной парой). Для регистров бухгалтерии берёт все запросы либо, с
`--acc-cap N`, N запросов на пару, взятых равномерно по длине текста —
от самого короткого до самого длинного. Порядок исходного файла
сохраняется.

    select_coverage.py <dump>/corpus_all.jsonl <dump>/corpus.jsonl --cap 2 --acc-cap 12
"""

import argparse
import collections
import json
import re

REGISTER_TABLE = re.compile(
    r"(РегистрНакопления|AccumulationRegister|AccumRg|РегистрБухгалтерии|AccountingRegister|AccRg)"
    r"\s*\.\s*([\wА-Яа-яЁё]+)(?:\s*\.\s*([\wА-Яа-яЁё]+))?",
    re.IGNORECASE,
)


def keys(text: str):
    found = set()
    for kind, name, virtual in REGISTER_TABLE.findall(text):
        accounting = kind.lower().startswith(("регистрбух", "accounting", "accrg"))
        found.add(("acc" if accounting else "accum", name.lower(), (virtual or "<main>").lower()))
    return found


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("source")
    parser.add_argument("target")
    parser.add_argument("--cap", type=int, default=2)
    parser.add_argument("--acc-cap", type=int, default=0, help="0 — все запросы к регистрам бухгалтерии")
    args = parser.parse_args()
    entries = [json.loads(line) for line in open(args.source, encoding="utf-8") if line.strip()]
    count = collections.Counter()
    chosen = set()
    by_length = sorted(range(len(entries)), key=lambda i: len(entries[i]["text"]))
    if args.acc_cap:
        # Every accounting key gets an even spread of its queries by length.
        per_key = collections.defaultdict(list)
        for index in by_length:
            for key in keys(entries[index]["text"]):
                if key[0] == "acc":
                    per_key[key].append(index)
        for key, indexes in per_key.items():
            take = min(args.acc_cap, len(indexes))
            for position in range(take):
                chosen.add(indexes[position * len(indexes) // take])
    for index in by_length:
        found = keys(entries[index]["text"])
        accounting = any(key[0] == "acc" for key in found)
        if (accounting and not args.acc_cap) or any(
            key[0] == "accum" and count[key] < args.cap for key in found
        ):
            chosen.add(index)
            for key in found:
                count[key] += 1
    with open(args.target, "w", encoding="utf-8") as out:
        for index, entry in enumerate(entries):
            if index in chosen:
                out.write(json.dumps(entry, ensure_ascii=False) + "\n")
    print(f"selected {len(chosen)} of {len(entries)} queries covering {len(count)} register tables")


if __name__ == "__main__":
    main()
