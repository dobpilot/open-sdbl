#!/usr/bin/env python3
"""Adds to a fixture corpus a sample of the queries that read no register.

The register corpora cover the virtual tables; this sample covers the rest
of the language — catalogs, documents, information registers, totals,
nested queries — by taking, per first-named metadata object, the shortest
query that names no accumulation or accounting register, up to `--cap`
queries in all, shortest first. Entries already in the target are kept.

Usage: select_wide.py <full corpus_all.jsonl> <fixture corpus.jsonl> --cap 200
"""
import argparse
import json
import re

REGISTER = re.compile(r"Регистр(Накопления|Бухгалтерии)\.", re.I)
FIRST = re.compile(r"\b(?:ИЗ|FROM)\s+((?:Справочник|Документ|РегистрСведений|Перечисление|ПланСчетов|ПланВидовХарактеристик|ПланВидовРасчета|ПланОбмена|Задача|БизнесПроцесс|ЖурналДокументов|Константы|КритерийОтбора)\.\w+)", re.I)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("source")
    parser.add_argument("target")
    parser.add_argument("--cap", type=int, default=200)
    args = parser.parse_args()
    existing = [line for line in open(args.target, encoding="utf-8").read().splitlines() if line.strip()]
    seen = {json.loads(line)["source"] for line in existing}
    seen_text = {json.loads(line)["text"] for line in existing}
    best = {}
    for line in open(args.source, encoding="utf-8"):
        if not line.strip():
            continue
        entry = json.loads(line)
        text = entry["text"]
        if REGISTER.search(text) or entry["source"] in seen or text in seen_text:
            continue
        match = FIRST.search(text)
        if not match:
            continue
        key = match.group(1).lower()
        if key not in best or len(text) < len(best[key]["text"]):
            best[key] = entry
    chosen = sorted(best.values(), key=lambda entry: len(entry["text"]))[: args.cap]
    with open(args.target, "a", encoding="utf-8") as out:
        for entry in chosen:
            out.write(json.dumps({"source": entry["source"], "text": entry["text"]}, ensure_ascii=False, separators=(",", ":")) + "\n")
    print(f"added {len(chosen)} of {len(best)} candidates")


if __name__ == "__main__":
    main()
