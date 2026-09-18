#!/usr/bin/env python3
"""Binds the value-table parameters of a corpus fixture.

A query reading `ИЗ &Таблица КАК Т` (or joining it) needs a table value to
compile; the corpus has no data, only the text, so each such parameter is
bound to an empty table whose columns are the fields the query reads as
`Т.Поле`, each with a kind inferred from the text: `Т.Поле ССЫЛКА Вид.Х`
or `ВЫРАЗИТЬ(Т.Поле КАК Вид.Х)` name the object, an equality with the
`Ссылка` of a metadata source names its object, the column name tells a
number, a date, a string or a boolean, and anything else is a universal
reference. Recorded as the `T<col>:<kind>,…` literal in `params`.

With `--kinds <tsv>` (written by the ignored test `dump_field_kinds` of
`tests/query_corpus.rs`) a column compared, joined or tuple-matched with a
field of a metadata table — a register dimension in a virtual table
condition included — takes that field's kind.

Usage: bind_tables.py tests/fixtures/<name> [--kinds <tsv>]
"""
import json
import re
import sys
from pathlib import Path

KEYWORDS = {"КАК", "AS", "ГДЕ", "WHERE", "ВНУТРЕННЕЕ", "ЛЕВОЕ", "ПРАВОЕ", "ПОЛНОЕ",
            "СОЕДИНЕНИЕ", "INNER", "LEFT", "RIGHT", "FULL", "JOIN", "СГРУППИРОВАТЬ",
            "GROUP", "УПОРЯДОЧИТЬ", "ORDER", "ИТОГИ", "TOTALS", "ПО", "ON", "ОБЪЕДИНИТЬ",
            "UNION", "ИМЕЮЩИЕ", "HAVING", "ИНДЕКСИРОВАТЬ", "INDEX", "ДЛЯ", "FOR"}
SOURCE = re.compile(r"\b(?:ИЗ|FROM|СОЕДИНЕНИЕ|JOIN)\s+&(\w+)(?:\s+(?:КАК|AS)\s+(\w+)|\s+(\w+))?", re.I)
KINDS = ("Справочник", "Документ", "Перечисление", "ПланВидовХарактеристик", "ПланСчетов",
         "ПланВидовРасчета", "БизнесПроцесс", "Задача", "ПланОбмена")
OBJECT = r"(?:" + "|".join(KINDS) + r")\.\w+"
TABLE_KINDS = KINDS + ("РегистрНакопления", "РегистрСведений", "РегистрБухгалтерии",
                       "РегистрРасчета", "ЖурналДокументов")
TABLE = re.compile(r"\b(" + "|".join(TABLE_KINDS) + r")\.(\w+)(?:\.(\w+))?", re.I)
# The kinds `tools/corpus` reads from the fixture: `(объект, поле)` →
# tag, lower-cased keys. Empty without `--kinds`.
FIELD_KINDS: dict[tuple[str, str], str] = {}
RESOURCE_SUFFIXES = ("НачальныйОстаток", "КонечныйОстаток", "РазвернутыйОстатокДт",
                     "РазвернутыйОстатокКт", "ОстатокДт", "ОстатокКт", "ОборотДт", "ОборотКт",
                     "Остаток", "Приход", "Расход", "Оборот", "Дт", "Кт")
NUMBER = re.compile(r"^(НомерСтроки|\w*Номер|Количество\w*|Сумма\w*|Цена\w*|Коэффициент\w*|Процент\w*|Ставка\w*|Курс|Кратность|Порядок|Индекс\w*|Вес\w*|Объем\w*|Длительность\w*|Год|Месяц|День|Квартал)$", re.I)
DATE = re.compile(r"^(Дата\w*|Период\w*|Месяц\w+|\w*Дата|\w*Период|\w*Периода|Начало\w*|Конец\w*|Время\w*)$", re.I)
STRING = re.compile(r"^(Код|Наименование\w*|\w*Наименование|Артикул|Комментарий|Содержание|Представление\w*|\w*Представление|Описание|Текст\w*|Имя\w*|ИНН|КПП|ОКПО|Адрес\w*|Телефон\w*|Строка\w*|\w*Строкой)$", re.I)
BOOLEAN = re.compile(r"^(Использовать\w*|Признак\w*|Активность|\w*Флаг|Есть\w*|Это\w*|Разрешено\w*|Включено\w*|Учитывать\w*|Проведен|Пометка\w*)$", re.I)


def metadata_aliases(text: str) -> dict[str, str]:
    """Aliases of metadata sources, lower-cased, to `Вид.Объект`."""
    aliases = {}
    for match in re.finditer(r"(" + OBJECT + r")(?:\.\w+)*\s+(?:КАК\s+)?(\w+)", text):
        alias = match.group(2)
        if alias.upper() in KEYWORDS:
            continue
        aliases[alias.lower()] = match.group(1)
    return aliases


def balanced_end(text: str, start: int) -> int:
    """Index after the parenthesis group opening at `start`."""
    depth = 0
    for index in range(start, len(text)):
        if text[index] == "(":
            depth += 1
        elif text[index] == ")":
            depth -= 1
            if depth == 0:
                return index + 1
    return len(text)


def table_sources(text: str) -> list[dict]:
    """Every metadata table the text reads: object, virtual table, alias,
    and the span of the virtual table's arguments."""
    sources = []
    for match in TABLE.finditer(text):
        kind, name, third = match.group(1), match.group(2), match.group(3)
        obj = f"{kind}.{name}"
        end = match.end()
        virtual = None
        span = None
        rest = text[end:]
        if third and re.match(r"\s*\(", rest):
            virtual = third
            arguments_start = end + rest.index("(")
            span = (arguments_start, balanced_end(text, arguments_start))
            end = span[1]
        elif third:
            obj = f"{obj}.{third}"
        alias = re.match(r"\s+(?:(?:КАК|AS)\s+)?(\w+)", text[end:], re.I)
        alias = alias.group(1) if alias and alias.group(1).upper() not in KEYWORDS else None
        sources.append({"object": obj, "virtual": virtual, "alias": alias or name, "span": span})
    return sources


def field_kind(obj: str, virtual: str | None, field: str) -> str | None:
    """The tag of a field of a metadata table, a virtual one included."""
    if virtual:
        if field.lower() == "период":
            return "ДАТА"
        if field.lower() in ("регистратор", "моментвремени"):
            return None
    kind = FIELD_KINDS.get((obj.lower(), field.lower()))
    if kind or not virtual:
        return kind
    for suffix in RESOURCE_SUFFIXES:
        if field.lower().endswith(suffix.lower()) and len(field) > len(suffix):
            kind = FIELD_KINDS.get((obj.lower(), field[: -len(suffix)].lower()))
            if kind:
                return kind
    return None


def metadata_kind(text: str, alias: str, column: str) -> str | None:
    """The kind of a placeholder column compared with a metadata field:
    `X.col = A.f`, `A.f В (ВЫБРАТЬ X.col …)`, a bare register field in a
    virtual table condition, or a tuple `(f1, f2) В (ВЫБРАТЬ X.c1, X.c2 …)`."""
    if not FIELD_KINDS:
        return None
    sources = table_sources(text)
    by_alias = {}
    for source in sources:
        by_alias.setdefault(source["alias"].lower(), source)
    field = re.escape(alias) + r"\." + re.escape(column) + r"\b"

    def enclosing(position: int) -> dict | None:
        for source in sources:
            if source["span"] and source["span"][0] <= position < source["span"][1]:
                return source
        return None

    def kind_of(qualifier: str | None, name: str, position: int) -> str | None:
        if qualifier:
            source = by_alias.get(qualifier.lower())
            return field_kind(source["object"], source["virtual"], name) if source else None
        source = enclosing(position)
        return field_kind(source["object"], None, name) if source else None

    qualified = r"(?:(\w+)\.)?(\w+)"
    for pattern, qualifier_group, name_group in (
        (field + r"\s*=\s*(?<![\w.])" + qualified + r"\b(?!\s*\()", 1, 2),
        (r"(?<![\w.])" + qualified + r"\s*=\s*" + field, 1, 2),
        (r"(?<![\w.])" + qualified + r"\s+(?:В|IN)\s*\(\s*(?:ВЫБРАТЬ|SELECT)\s+(?:РАЗЛИЧНЫЕ\s+|DISTINCT\s+)?" + field, 1, 2),
    ):
        for match in re.finditer(pattern, text, re.I):
            qualifier, name = match.group(qualifier_group), match.group(name_group)
            if (qualifier or "").lower() == alias.lower() or name.upper() in KEYWORDS:
                continue
            if qualifier is None and (name.lower() in ("null", "истина", "ложь", "неопределено") or name[0].isdigit()):
                continue
            kind = kind_of(qualifier, name, match.start())
            if kind:
                return kind
    # A tuple: the placeholder column takes the kind of the field at its position.
    tuple_pattern = r"\(([\w\s,.]+?)\)\s+(?:В|IN)\s*\(\s*(?:ВЫБРАТЬ|SELECT)\s+(?:РАЗЛИЧНЫЕ\s+|DISTINCT\s+)?(.*?)\s+(?:ИЗ|FROM)\b"
    for match in re.finditer(tuple_pattern, text, re.I | re.S):
        left = [item.strip() for item in match.group(1).split(",")]
        right = [re.sub(r"\s+(?:КАК|AS)\s+\w+$", "", item.strip(), flags=re.I) for item in match.group(2).split(",")]
        if len(left) != len(right):
            continue
        for item, projected in zip(left, right):
            if projected.lower() != f"{alias}.{column}".lower():
                continue
            qualifier, _, name = item.rpartition(".")
            kind = kind_of(qualifier or None, name, match.start())
            if kind:
                return kind
    return None


def infer_kind(text: str, alias: str, column: str, aliases: dict[str, str]) -> str:
    kind = typed_kind(text, alias, column, aliases) or metadata_kind(text, alias, column) or \
        context_kind(text, alias, column, aliases)
    if kind:
        return kind
    # A temporary table that projects the column carries its uses on: the
    # kind read there is the kind here.
    for table_alias, projected in derived_uses(text, alias, column):
        kind = context_kind(text, table_alias, projected, aliases)
        if kind:
            return kind
    return name_kind(column)


def derived_uses(text: str, alias: str, column: str) -> list[tuple[str, str]]:
    """`(alias, column)` pairs under which temporary tables expose the column."""
    uses = []
    field = re.escape(alias) + r"\." + re.escape(column) + r"\b"
    for statement in text.split(";"):
        table = DEFINED.search(statement)
        if not table:
            continue
        projected = re.search(field + r"(?:\s+(?:КАК|AS)\s+(\w+))?\s*(?:,|ПОМЕСТИТЬ|INTO)", statement, re.I)
        if not projected:
            continue
        name = projected.group(1) or column
        for source in TEMP_SOURCE.finditer(text):
            if source.group(1).lower() != table.group(1).lower():
                continue
            table_alias = source.group(2) or source.group(3) or table.group(1)
            if table_alias.upper() in KEYWORDS:
                table_alias = table.group(1)
            uses.append((table_alias, name))
    return uses


def typed_kind(text: str, alias: str, column: str, aliases: dict[str, str]) -> str | None:
    """`Т.Поле ССЫЛКА Вид.Х`, `ВЫРАЗИТЬ(Т.Поле КАК Вид.Х)` or an equality
    with the `Ссылка` of a metadata source name the object outright."""
    field = re.escape(alias) + r"\." + re.escape(column) + r"\b"
    typed = re.search(field + r"\s+ССЫЛКА\s+(" + OBJECT + ")", text, re.I) or \
        re.search(r"ВЫРАЗИТЬ\s*\(\s*" + field + r"\s+КАК\s+(" + OBJECT + ")", text, re.I)
    if typed:
        return typed.group(1)
    for pattern in (field + r"\s*=\s*(\w+)\.Ссылка\b", r"(\w+)\.Ссылка\s*=\s*" + field):
        for match in re.finditer(pattern, text, re.I):
            target = aliases.get(match.group(1).lower())
            if target:
                return target
    return None


def context_kind(text: str, alias: str, column: str, aliases: dict[str, str]) -> str | None:
    field = re.escape(alias) + r"\." + re.escape(column) + r"\b"
    typed = re.search(field + r"\s+ССЫЛКА\s+(" + OBJECT + ")", text, re.I) or \
        re.search(r"ВЫРАЗИТЬ\s*\(\s*" + field + r"\s+КАК\s+(" + OBJECT + ")", text, re.I)
    if typed:
        return typed.group(1)
    for pattern in (field + r"\s*=\s*(\w+)\.Ссылка\b", r"(\w+)\.Ссылка\s*=\s*" + field):
        for match in re.finditer(pattern, text, re.I):
            target = aliases.get(match.group(1).lower())
            if target:
                return target
    # The context of the field tells its kind before its name does.
    after = field + r"\s*"
    before = r"(?<![\w.])"
    # The other branch of a `ВЫБОР` tells the kind of this one.
    for literal, kind in ((r"-?\d[\d.]*", "ЧИСЛО"), (r"\"[^\"]*\"", "СТРОКА"),
                          (r"ДАТАВРЕМЯ\s*\([^)]*\)", "ДАТА"), (r"(ИСТИНА|ЛОЖЬ|TRUE|FALSE)\b", "БУЛЕВО")):
        if re.search(r"(ТОГДА|THEN)\s+" + literal + r"\s+(ИНАЧЕ|ELSE)\s+" + field, text, re.I) or \
                re.search(r"(ТОГДА|THEN)\s+" + field + r"\s+(ИНАЧЕ|ELSE)\s+" + literal, text, re.I):
            return kind
    if re.search(after + r"(=|<>|>=|<=|>|<)\s*-?\d", text) or \
            re.search(r"(СУММА|SUM|СРЕДНЕЕ|AVG)\s*\(\s*" + field, text, re.I) or \
            re.search(after + r"[-+*/]\s*[\w(]", text) or re.search(r"[-+*/]\s*" + field, text) or \
            re.search(r"ЕСТЬNULL\s*\(\s*" + field + r"\s*,\s*-?\d", text, re.I) or \
            re.search(r"(ВЫРАЗИТЬ|CAST)\s*\(\s*" + field + r"\s+КАК\s+ЧИСЛО", text, re.I):
        return "ЧИСЛО"
    if re.search(after + r"(=|<>)\s*(ИСТИНА|ЛОЖЬ|TRUE|FALSE)\b", text, re.I) or \
            re.search(r"\b(НЕ|NOT)\s+" + field + r"(?!\s*(ЕСТЬ|IS)\b)", text, re.I) or \
            re.search(r"ЕСТЬNULL\s*\(\s*" + field + r"\s*,\s*(ИСТИНА|ЛОЖЬ)", text, re.I) or \
            re.search(r"\b(И|AND|ГДЕ|WHERE|КОГДА|WHEN|ИЛИ|OR)\s+" + field + r"\s*(И|AND|ИЛИ|OR|ТОГДА|THEN|\)|$)", text, re.I):
        return "БУЛЕВО"
    if re.search(r"(НАЧАЛОПЕРИОДА|КОНЕЦПЕРИОДА|ДОБАВИТЬКДАТЕ|РАЗНОСТЬДАТ|ГОД|МЕСЯЦ|ДЕНЬ|КВАРТАЛ)\s*\(\s*" + field, text, re.I) or \
            re.search(after + r"(=|<>|>=|<=|>|<|МЕЖДУ)\s*(ДАТАВРЕМЯ|&\w*(Дат|Период|Начал|Конец)|\w+\.(Дата\w*|Период\w*))", text, re.I) or \
            re.search(r"ЕСТЬNULL\s*\(\s*" + field + r"\s*,\s*ДАТАВРЕМЯ", text, re.I):
        return "ДАТА"
    if re.search(after + r"(ПОДОБНО|LIKE)\b", text, re.I) or \
            re.search(r"(ПОДСТРОКА|ДЛИНАСТРОКИ|СОКРЛП|ВРЕГ|НРЕГ|СТРНАЙТИ)\s*\(\s*" + field, text, re.I) or \
            re.search(after + r"(=|<>)\s*\"", text) or re.search(r"ЕСТЬNULL\s*\(\s*" + field + r"\s*,\s*\"", text, re.I):
        return "СТРОКА"
    return None


def name_kind(column: str) -> str:
    if NUMBER.match(column) or re.search(r"(^|_)(НДС|БезНДС|СНДС|Сумма\w*)$", column, re.I):
        return "ЧИСЛО"
    if DATE.match(column):
        return "ДАТА"
    if STRING.match(column):
        return "СТРОКА"
    # `НеПроводить`, `НеУчитывать` — but not `НематериальныйАктив`.
    if BOOLEAN.match(column) or (column[:2].lower() == "не" and column[2:3].isupper()):
        return "БУЛЕВО"
    return "ЛЮБАЯССЫЛКА"


TEMP_SOURCE = re.compile(r"\b(?:ИЗ|FROM|СОЕДИНЕНИЕ|JOIN)\s+([A-Za-zА-Яа-я_]\w*)\b(?!\s*[.(])(?:\s+(?:КАК|AS)\s+(\w+)|\s+(\w+))?", re.I)
DEFINED = re.compile(r"\b(?:ПОМЕСТИТЬ|INTO)\s+(\w+)", re.I)


def columns_of(text: str, alias: str, aliases: dict[str, str]) -> str:
    columns = []
    for field in re.finditer(r"\b" + re.escape(alias) + r"\.(\w+)", text):
        name = field.group(1)
        if name.lower() not in (c.lower() for c, _ in columns):
            columns.append((name, infer_kind(text, alias, name, aliases)))
    return "T" + ",".join(f"{name}:{kind}" for name, kind in columns)


def bindings(text: str) -> dict[str, str]:
    result = {}
    aliases = metadata_aliases(text)
    for match in SOURCE.finditer(text):
        parameter = match.group(1)
        alias = match.group(2) or match.group(3) or parameter
        if alias.upper() in KEYWORDS:
            alias = parameter
        result[parameter] = columns_of(text, alias, aliases)
    # A temporary table the text reads but never defines comes from another
    # batch: the runner defines it from a `__vt_<name>` table parameter
    # whose columns are the fields the text reads.
    defined = {name.lower() for name in DEFINED.findall(text)}
    for match in TEMP_SOURCE.finditer(text):
        name = match.group(1)
        if name.upper() in KEYWORDS or name.lower() in defined or name.lower() == "константы":
            continue
        alias = match.group(2) or match.group(3) or name
        if alias.upper() in KEYWORDS:
            alias = name
        key = f"__vt_{name}"
        columns = columns_of(text, alias, aliases)
        result[key] = merge_columns(result.get(key, "T"), columns)
    return result


def merge_columns(first: str, second: str) -> str:
    """Joins two `T…` column lists, keeping the first kind of a repeated name."""
    columns = []
    for spec in (first, second):
        for item in spec[1:].split(","):
            if item and item.split(":")[0].lower() not in (c.split(":")[0].lower() for c in columns):
                columns.append(item)
    return "T" + ",".join(columns)


def main(root: Path) -> None:
    path = root / "corpus.jsonl"
    lines = path.read_text(encoding="utf-8").splitlines()
    bound = 0
    out = []
    for line in lines:
        if not line.strip():
            out.append(line)
            continue
        entry = json.loads(line)
        params = entry.get("params", {})
        changed = False
        # Table bindings are recomputed from the text: a stale one goes.
        fresh = bindings(entry["text"])
        for name in [name for name, literal in params.items() if literal.startswith("T") and name not in fresh]:
            del params[name]
            changed = True
        for name, literal in fresh.items():
            if params.get(name) != literal:
                params[name] = literal
                changed = True
        if changed:
            entry["params"] = params
            bound += 1
        out.append(json.dumps(entry, ensure_ascii=False, separators=(",", ":")))
    path.write_text("\n".join(out) + "\n", encoding="utf-8")
    print(f"bound {bound}")


def load_kinds(path: Path) -> None:
    for line in path.read_text(encoding="utf-8").splitlines():
        parts = line.split("\t")
        if len(parts) == 3:
            FIELD_KINDS[(parts[0].lower(), parts[1].lower())] = parts[2]


if __name__ == "__main__":
    arguments = sys.argv[1:]
    if "--kinds" in arguments:
        at = arguments.index("--kinds")
        load_kinds(Path(arguments[at + 1]))
        del arguments[at:at + 2]
    main(Path(arguments[0]))
