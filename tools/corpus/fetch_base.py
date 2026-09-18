#!/usr/bin/env python3
"""Снимает с базы 1С на PostgreSQL всё, что нужно корпусу запросов.

Читает только четыре таблицы платформы (`params`, `config`, `schemastorage`,
`information_schema.columns`) в транзакции только для чтения и пишет в
каталог `--out`:

  db_names.deflate    таблица DBNames целиком (сырой deflate, части склеены)
  config.pack         записи Config с описателями и предопределёнными
                      значениями (`.1c` справочников, `.9` планов счетов, `.7` планов
                      видов характеристик):
                      `<ресурс>\\t<длина>\\n` + сырые deflate-байты
  schema_storage.txt  SchemaStorage целиком
  live_columns.tsv    колонки всех живых таблиц: таблица, колонка, тип
  corpus.jsonl        запросы, найденные в модулях и схемах компоновки:
                      {"source": "<ресурс Config>", "text": "<текст>"}
  summary.json        сколько чего нашлось и какие виртуальные таблицы
                      регистров употребляются

Полный дамп потом обрезается до фикстуры:
`CORPUS_FULL=<out> CORPUS_OUT=tests/fixtures/<имя> cargo test -p open-sdbl
--test corpus_fixture -- --ignored`.

Подключение — как у psql: `--dsn "host=localhost dbname=unf user=…"` либо
переменные PGHOST/PGDATABASE/PGUSER/PGPASSWORD/PGPASSFILE. Пароль в
аргументах не передаётся.
"""

from __future__ import annotations

import argparse
import html
import json
import os
import re
import sys
import zlib
from collections import Counter

import psycopg2

GUID = r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}"
PACK_RESOURCE = re.compile(rf"^{GUID}(\.1c|\.9|\.7)?$")
QUERY_START = re.compile(r"^\s*(ВЫБРАТЬ|SELECT)\b", re.IGNORECASE)
DCS_QUERY = re.compile(r"<query(?:\s[^>]*)?>(.*?)</query>", re.DOTALL | re.IGNORECASE)
REGISTER_TABLE = re.compile(
    r"(РегистрНакопления|AccumulationRegister|AccumRg|РегистрБухгалтерии|AccountingRegister|AccRg)"
    r"\s*\.\s*([A-Za-zА-Яа-яЁё_][\wА-Яа-яЁё]*)(?:\s*\.\s*([A-Za-zА-Яа-яЁё_][\wА-Яа-яЁё]*))?",
    re.IGNORECASE,
)
INFLATE_LIMIT = 256 * 1024 * 1024


def inflate(data: bytes) -> bytes:
    """Raw deflate без заголовка zlib — так платформа хранит Config."""
    return zlib.decompressobj(-15).decompress(data, INFLATE_LIMIT)


def decode_text(data: bytes) -> str:
    if data.startswith(b"\xff\xfe"):
        return data[2:].decode("utf-16-le", "replace")
    if data.startswith(b"\xfe\xff"):
        return data[2:].decode("utf-16-be", "replace")
    if data.startswith(b"\xef\xbb\xbf"):
        data = data[3:]
    return data.decode("utf-8", "replace")


def neighbour(text: str, index: int, step: int) -> str:
    """Ближайший непробельный символ от `index` в сторону `step`."""
    while 0 <= index < len(text) and text[index] in " \t\r\n":
        index += step
    return text[index] if 0 <= index < len(text) else ""


def string_literals(text: str):
    """Строковые литералы встроенного языка, включая многострочные с `|`.

    Отдаёт пары `(текст, фрагмент)`: фрагмент — литерал, склеенный с
    соседом через `+`; такой текст не является целым запросом.
    """
    i, n = 0, len(text)
    while i < n:
        if text[i] != '"':
            i += 1
            continue
        fragment = neighbour(text, i - 1, -1) == "+"
        i += 1
        parts = []
        start = i
        while i < n:
            c = text[i]
            if c == '"':
                if i + 1 < n and text[i + 1] == '"':
                    parts.append(text[start:i] + '"')
                    i += 2
                    start = i
                    continue
                parts.append(text[start:i])
                i += 1
                fragment = fragment or neighbour(text, i, 1) == "+"
                break
            if c == "\n" or c == "\r":
                parts.append(text[start:i])
                j = i
                if c == "\r" and j + 1 < n and text[j + 1] == "\n":
                    j += 1
                j += 1
                while j < n and text[j] in " \t":
                    j += 1
                if j < n and text[j] == "|":
                    parts.append("\r\n")
                    i = j + 1
                    start = i
                    continue
                # Перевод строки без `|` — литерал закончился (или это не код).
                i = j
                break
            i += 1
        else:
            parts.append(text[start:i])
        yield "".join(parts), fragment


PLACEHOLDER = re.compile(r"%\d")


def queries_in(text: str):
    for literal, fragment in string_literals(text):
        if not QUERY_START.match(literal):
            continue
        # Склейка через `+`, подстановка `%1` и незакрытые скобки — это
        # заготовка текста, а не запрос.
        if fragment or PLACEHOLDER.search(literal) or literal.count("(") != literal.count(")"):
            continue
        yield literal
    for match in DCS_QUERY.finditer(text):
        body = html.unescape(match.group(1))
        if QUERY_START.match(body):
            yield body


def normalize(text: str) -> str:
    return re.sub(r"\s+", " ", text).strip().lower()


class Extractor:
    def __init__(self, mode: str):
        self.mode = mode
        self.seen: set[str] = set()
        self.entries: list[dict] = []
        self.tables = Counter()
        self.scanned = 0
        self.dropped_composition = 0

    def feed(self, source: str, text: str) -> None:
        self.scanned += 1
        for query in queries_in(text):
            query = query.strip("﻿ \t\r\n")
            if "{" in query:
                self.dropped_composition += 1
                continue
            uses = REGISTER_TABLE.findall(query)
            if self.mode == "registers" and not uses:
                continue
            key = normalize(query)
            if key in self.seen:
                continue
            self.seen.add(key)
            for kind, name, virtual in uses:
                self.tables[f"{kind}.{name}" + (f".{virtual}" if virtual else "")] += 1
            self.entries.append({"source": source, "text": query})


def has_column(cur, table: str, column: str) -> bool:
    cur.execute(
        "SELECT 1 FROM information_schema.columns WHERE table_schema='public' "
        "AND table_name=%s AND column_name=%s",
        (table, column),
    )
    return cur.fetchone() is not None


def as_bytes(value) -> bytes:
    if isinstance(value, memoryview):
        return value.tobytes()
    if isinstance(value, str):
        return value.encode("utf-8")
    return bytes(value)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--dsn", default="", help="строка libpq; пусто — переменные PG*")
    parser.add_argument("--out", required=True, help="каталог полного дампа")
    parser.add_argument("--queries", default="registers", choices=["registers", "all"],
                        help="registers — только запросы к регистрам накопления и бухгалтерии")
    args = parser.parse_args()

    os.makedirs(args.out, exist_ok=True)
    conn = psycopg2.connect(args.dsn)
    conn.set_session(readonly=True, autocommit=False)
    cur = conn.cursor()

    params_parts = has_column(cur, "params", "partno")
    config_parts = has_column(cur, "config", "partno")
    print(f"layout: params.partno={params_parts} config.partno={config_parts}", file=sys.stderr)

    # DBNames
    if params_parts:
        cur.execute("SELECT partno, binarydata FROM params WHERE rtrim(filename::text)='DBNames' ORDER BY partno")
    else:
        cur.execute("SELECT 0, binarydata FROM params WHERE rtrim(filename::text)='DBNames'")
    db_names = b"".join(as_bytes(row[1]) for row in cur.fetchall())
    with open(os.path.join(args.out, "db_names.deflate"), "wb") as f:
        f.write(db_names)
    print(f"DBNames: {len(db_names)} bytes", file=sys.stderr)

    # SchemaStorage
    cur.execute("SELECT currentschema FROM schemastorage WHERE schemaid = 0")
    schema = as_bytes(cur.fetchone()[0])
    if not schema.lstrip(b"\xef\xbb\xbf").startswith(b"{"):
        schema = inflate(schema)
    with open(os.path.join(args.out, "schema_storage.txt"), "wb") as f:
        f.write(schema)
    print(f"SchemaStorage: {len(schema)} bytes", file=sys.stderr)

    # Live columns
    cur.execute(
        "SELECT table_name, column_name, data_type FROM information_schema.columns "
        "WHERE table_schema='public' ORDER BY table_name, ordinal_position"
    )
    with open(os.path.join(args.out, "live_columns.tsv"), "w", encoding="utf-8") as f:
        rows = 0
        for table, column, data_type in cur:
            f.write(f"{table}\t{column}\t{data_type}\n")
            rows += 1
    print(f"live columns: {rows}", file=sys.stderr)

    # Config: pack of descriptor resources + query texts of everything else.
    extractor = Extractor(args.queries)
    server = conn.cursor(name="config_scan")
    server.itersize = 200
    if config_parts:
        server.execute("SELECT rtrim(filename::text), partno, binarydata FROM config ORDER BY filename, partno")
    else:
        server.execute("SELECT rtrim(filename::text), 0, binarydata FROM config ORDER BY filename")
    packed = 0
    inflate_failures = 0
    with open(os.path.join(args.out, "config.pack"), "wb") as pack:
        current = None
        parts: list[bytes] = []

        def flush():
            nonlocal packed, inflate_failures
            if current is None:
                return
            data = b"".join(parts)
            if PACK_RESOURCE.match(current):
                pack.write(f"{current.lower()}\t{len(data)}\n".encode("utf-8"))
                pack.write(data)
                packed += 1
                return
            try:
                text = decode_text(inflate(data))
            except zlib.error:
                inflate_failures += 1
                return
            extractor.feed(current, text)

        for name, _part, data in server:
            if name != current:
                flush()
                current, parts = name, []
            parts.append(as_bytes(data))
        flush()
    conn.rollback()
    print(f"Config: packed {packed} descriptor resources, scanned {extractor.scanned} others, "
          f"{inflate_failures} not deflate", file=sys.stderr)

    with open(os.path.join(args.out, "corpus.jsonl"), "w", encoding="utf-8") as f:
        for entry in extractor.entries:
            f.write(json.dumps(entry, ensure_ascii=False) + "\n")
    summary = {
        "queries": len(extractor.entries),
        "dropped_composition_texts": extractor.dropped_composition,
        "register_tables": dict(sorted(extractor.tables.items(), key=lambda item: (-item[1], item[0]))),
    }
    with open(os.path.join(args.out, "summary.json"), "w", encoding="utf-8") as f:
        json.dump(summary, f, ensure_ascii=False, indent=2)
    print(f"queries: {len(extractor.entries)} (composition texts dropped: {extractor.dropped_composition})",
          file=sys.stderr)
    for table, count in summary["register_tables"].items():
        print(f"  {count:4d}  {table}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
