# open-sdbl

[![CI](https://github.com/dobpilot/open-sdbl/actions/workflows/ci.yml/badge.svg)](https://github.com/dobpilot/open-sdbl/actions/workflows/ci.yml)
[![Rust 2024](https://img.shields.io/badge/Rust-2024-dea584?logo=rust)](https://doc.rust-lang.org/edition-guide/rust-2024/)
[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0--only-blue.svg)](LICENSE)

`open-sdbl` — библиотека и интерактивная консоль для запросов к информационным
базам 1С на PostgreSQL. Проект читает служебные метаданные 1С, связывает имена
объектов и реквизитов с физической схемой, а затем преобразует запросы SDBL в
SQL.

Ядро `open-sdbl` не выполняет I/O и не имеет production-зависимостей. Оно
декодирует переданные приложением `DBNames`, `Config` и `SchemaStorage`, строит
снимок метаданных и генерирует SQL. Подключение к PostgreSQL, read-only
транзакции, интерактивный терминал и кеш находятся в отдельном приложении
`open-sdbl-cli`.

Сейчас поддерживаются SELECT-запросы, соединения и объединения, агрегаты,
разыменование ссылок, представления значений, а также виртуальные таблицы
регистров: срез первых/последних, остатки и обороты. Проект развивается и пока
не является полной заменой языка запросов платформы 1С.

## CLI в работе

Описание объекта показывает логическое имя, GUID, физическую таблицу,
реквизиты и индексы:

<img src="docs/img/readme-gifs/metadata-navigation.gif" width="1000" alt="Описание метаданных командой \d">

Перед исполнением консоль показывает сгенерированный PostgreSQL-запрос и
отдельно измеряет генерацию SQL и выполнение в СУБД:

<img src="docs/img/readme-gifs/query-execution.gif" width="1000" alt="Преобразование SDBL в PostgreSQL и выполнение запроса">

Виртуальные таблицы и представления ссылок компилируются с учётом реальных
метаданных информационной базы:

<img src="docs/img/readme-gifs/virtual-table.gif" width="1000" alt="Остатки регистра накопления и представление ссылки">

## Начать использовать

Требуется Rust 1.85 или новее:

```console
git clone https://github.com/dobpilot/open-sdbl.git
cd open-sdbl
cargo build --release
```

Обычная release-сборка из корня создаёт CLI в
`target/release/open-sdbl`. Для сборки только библиотеки используйте
`cargo build --release --package open-sdbl`.

Запуск консоли:

```console
PGPASSFILE="$HOME/.pgpass" ./target/release/open-sdbl console postgres \
  --host db.example.local \
  --database onec \
  --user reader
```

Пароль читается из `PGPASSWORD`, `PGPASSFILE` или `$HOME/.pgpass`. Команда
работает через `tokio-postgres`, не требует установленного `psql` и выполняет
запросы в проверенной read-only транзакции `READ COMMITTED`.

При запуске в терминале загрузка метаданных показывает progress bar с фазой,
числом ресурсов Config и объёмом сжатых данных. Config читается потоково и
декодируется параллельно на blocking-пуле Tokio с ограниченным числом задач.
Progress выводится в `stderr`, поэтому табличный вывод `metadata postgres` в
`stdout` можно по-прежнему безопасно перенаправлять или обрабатывать скриптом.

Если база доступна через SOCKS5-прокси (например, через `ssh -D`), добавьте
`--socks5-proxy 127.0.0.1:1080`. Прокси получает исходное имя из `--host` и
разрешает его на своей стороне. Сейчас поддерживается SOCKS5 без аутентификации;
пароль PostgreSQL по-прежнему читается только из источников выше.

Команда | Назначение
--- | ---
`\dt` | список таблиц и объектов метаданных
`\di` | список индексов
`\d <имя>` | реквизиты и индексы объекта
`\refresh` | перечитать метаданные
`\help` | справка
`\q` | выход

В консоли работают история по стрелкам, подсветка синтаксиса и Tab-дополнение
команд, ключевых слов, объектов, полей и виртуальных таблиц.

## Trino / CedrusData 476

Интеграция состоит из Rust-сервиса и тонкого read-only Java connector. Сервис
остаётся владельцем метаданных 1С, PostgreSQL SQL и bind-параметров; connector
собран строго с `io.trino:trino-spi:476` и передаёт projection, predicates и
`LIMIT`. Причина отказа от stock Thrift и границы компонентов описаны в
[`docs/trino-architecture.md`](docs/trino-architecture.md).

Минимальный запуск:

```bash
cargo build --release --package open-sdbl-trino

OPEN_SDBL_POSTGRES_HOST=db.example.local \
OPEN_SDBL_POSTGRES_DATABASE=onec \
OPEN_SDBL_POSTGRES_USERNAME=readonly_onec \
OPEN_SDBL_POSTGRES_PASSWORD='...' \
OPEN_SDBL_POSTGRES_TLS_MODE=require \
OPEN_SDBL_METADATA_CACHE_TTL=300 \
OPEN_SDBL_POSTGRES_POOL_SIZE=8 \
OPEN_SDBL_POSTGRES_CONNECT_TIMEOUT_MS=10000 \
OPEN_SDBL_POSTGRES_POOL_CREATE_TIMEOUT_MS=10000 \
OPEN_SDBL_POSTGRES_POOL_WAIT_TIMEOUT_MS=10000 \
OPEN_SDBL_STATEMENT_TIMEOUT_MS=60000 \
OPEN_SDBL_QUERY_TIMEOUT_MS=65000 \
OPEN_SDBL_LISTEN=0.0.0.0:8088 \
./target/release/open-sdbl-trino

# Сборка требует JDK 24 — это classfile baseline Trino 476.
cd trino-open-sdbl && mvn package
```

Скопируйте всё содержимое `trino-open-sdbl/target/plugin` в каталог
`plugin/open-sdbl` каждого Trino/CedrusData узла и создайте
`etc/catalog/onec.properties`:

```properties
connector.name=open_sdbl
open-sdbl.uri=http://open-sdbl-trino:8088
open-sdbl.request-timeout-ms=65000
```

Проверка:

```bash
# Локальный Trino 476 с автоматически собранным plugin:
./run_trino.sh

trino --execute 'SHOW SCHEMAS FROM onec'
trino --execute 'SHOW TABLES FROM onec."Справочник"'
trino --execute 'DESCRIBE onec."Справочник"."Контрагенты"'
trino --execute 'SELECT "Код", "Наименование" FROM onec."Справочник"."Контрагенты" LIMIT 10'
trino --execute 'SELECT "Код", "Наименование", "ИНН" FROM onec."Справочник"."Контрагенты" WHERE "ИНН" = '\''7701234567'\'' LIMIT 10'
```

SDBL-операции, которых нет в Trino SQL, доступны через
полиморфную read-only table function:

```sql
SELECT *
FROM TABLE(onec.system.sdbl(
    query => 'SELECT Контрагент, REFPRESENTATION(Контрагент)
              FROM Document.Поступление'
))
LIMIT 10;

SELECT *
FROM TABLE(onec.system.sdbl(
    query => 'SELECT Номенклатура, Цена, Период
              FROM InformationRegister.ЦеныПродажные.SliceLast()'
));

SELECT *
FROM TABLE(onec.system.sdbl(
    query => 'SELECT Номенклатура, ОстатокОстаток
              FROM AccumulationRegister.Остатки.Balance()'
));
```

Аргумент `query` парсится как SDBL, а не PostgreSQL SQL. Сервис
допускает только поддержанный `SELECT`, определяет схему результа
через PostgreSQL prepare без чтения строк и заново проверяет форму при
исполнении. Внешние projection и `LIMIT` пробрасываются в PostgreSQL;
внешние predicates пока остаются в Trino, так как их перенос внутрь
`SliceLast` или `Balance` может изменить семантику.

На уровне PostgreSQL последний запрос выбирает только три поля и содержит
параметризованные predicate и `LIMIT`. Для временной проверки включите
`OPEN_SDBL_LOG=open_sdbl_trino=debug`. Health endpoints: `/health`, `/ready`,
метрики: `/metrics`. Полный Kubernetes-пример приведён в
[`docs/cedrusdata-476.md`](docs/cedrusdata-476.md), стратегия split — в
[`docs/trino-splits.md`](docs/trino-splits.md).

Для compose-теста сначала загрузите обезличенную копию настоящей 1С-базы в
PostgreSQL (нужны `DBNames`, `Config` и `SchemaStorage`), соберите Java plugin и
запустите `docker compose -f docker-compose.integration.yml up --build`.
Read-only acceptance-команды собраны в `integration/run-trino-acceptance.sh`.

## Подключение `open-sdbl` к Rust-проекту

Пока crate не опубликован на crates.io, подключите Git-репозиторий или локальный
путь:

```toml
[dependencies]
open-sdbl = { git = "https://github.com/dobpilot/open-sdbl.git" }

# Для разработки рядом с репозиторием:
# open-sdbl = { path = "../open-sdbl" }
```

Приложение само получает бинарные ресурсы и каталоги СУБД, затем передаёт их в
ядро:

```rust
use open_sdbl::metadata::{
    LiveTable, MetadataError, MetadataSnapshot, parse_config_descriptors,
    parse_db_names, parse_schema_storage, resolve_metadata,
};

fn build_metadata(
    db_names_blob: &[u8],
    config_rows: &[(String, Vec<u8>)],
    schema_blob: &[u8],
    live_tables: Vec<LiveTable>,
) -> Result<MetadataSnapshot, MetadataError> {
    let db_names = parse_db_names(db_names_blob)?;
    let schema = parse_schema_storage(schema_blob)?;
    let mut descriptors = Vec::new();

    for (file_name, binary_data) in config_rows {
        descriptors.extend(parse_config_descriptors(file_name, binary_data)?);
    }

    Ok(resolve_metadata(
        db_names,
        descriptors,
        schema,
        live_tables,
    ))
}
```

Аргументы `build_metadata` читает ваше приложение; `config_rows` содержит
ресурсы с голым GUID в `FileName` и `PartNo = 0`. Готовые SELECT-only выражения
находятся в `PostgresMetadataQueries`; ядро намеренно не знает о сети, паролях
и async runtime. Полный вариант загрузки через `tokio-postgres` есть в
[`open-sdbl-cli`](crates/open-sdbl-cli/src/main.rs).

Для запроса без функций представления достаточно одного вызова:

```rust
use open_sdbl::{
    metadata::MetadataSnapshot,
    query::{CompiledQuery, QueryDiagnostic, compile_postgres_query},
};

fn compile(metadata: &MetadataSnapshot) -> Result<CompiledQuery, QueryDiagnostic> {
    let query = "ВЫБРАТЬ Код, Наименование ИЗ Справочник.Договоры";
    compile_postgres_query(query, metadata)
}
```

`CompiledQuery` содержит только SQL и имена выходных колонок. Исполнение SQL и
декодирование результата остаются ответственностью приложения.

## Callback ABI представлений

Представление ссылки зависит от прикладной политики: одному приложению нужен
`Наименование (Код)`, другому — другой шаблон или язык. Поэтому
`.Представление`, `ПРЕДСТАВЛЕНИЕССЫЛКИ()` и `ПРЕДСТАВЛЕНИЕ()` компилируются в две
фазы:

```text
SDBL + MetadataSnapshot
        │
        ▼
prepare_postgres_query()
        │
        ├── PresentationRequest { ObjectId/GUID возможных типов ссылок }
        │                                      │
        │                         callback приложения
        │                                      │
        ◄── PresentationPlan { FieldId[], структурированный шаблон }
        │
        ▼
PreparedPostgresQuery::compile()
        │
        ▼
CompiledQuery { sql, columns }
```

Контракт использует идентификаторы, а не имена:

Тип | Стабильное значение | Назначение
--- | --- | ---
`ObjectId` | 16 байт реального GUID 1С | возможный тип ссылочного значения
`AttributeId` | 16 байт реального GUID 1С | пользовательский реквизит
`StandardFieldId` | `#[repr(u32)]` | стандартное поле без GUID: код, наименование, номер, дата и другие
`FieldId` | `Metadata(AttributeId)` или `Standard(StandardFieldId)` | любое поле шаблона
`PresentationExpression` | `Field`, `Literal`, `Concat` | безопасное дерево выражения без сырого SQL

Lookup-методы `MetadataSnapshot::object_id()` и
`MetadataSnapshot::field_id()` преобразуют имя в ID при настройке политики.
Индексы снимка дают ожидаемый поиск O(1), не считая нормализации имени.

Пример callback-политики `Наименование (Код)`:

```rust
use open_sdbl::{
    metadata::{LookupError, MetadataSnapshot},
    query::{
        CompiledQuery, PresentationExpression, PresentationPlan,
        prepare_postgres_query,
    },
};

fn compile_with_presentations(
    source: &str,
    metadata: &MetadataSnapshot,
) -> Result<CompiledQuery, Box<dyn std::error::Error>> {
    let prepared = prepare_postgres_query(source, metadata)?;

    // Это callback приложения. Запрос содержит дедуплицированный набор GUID
    // всех возможных типов ссылок, найденных ядром в SDBL.
    let plans = prepared
        .presentation_request()
        .targets
        .iter()
        .map(|target| -> Result<PresentationPlan, LookupError> {
            let name = metadata.field_id(target.object, "Наименование")?;
            let code = metadata.field_id(target.object, "Код")?;

            Ok(PresentationPlan {
                object: target.object,
                fields: vec![name, code],
                expression: PresentationExpression::Concat(vec![
                    PresentationExpression::Field(name),
                    PresentationExpression::Literal(" (".into()),
                    PresentationExpression::Field(code),
                    PresentationExpression::Literal(")".into()),
                ]),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(prepared.compile(metadata, &plans)?)
}
```

Ядро проверяет, что на каждый запрошенный `ObjectId` получен ровно один план,
все `FieldId` действительно принадлежат объекту, а шаблон использует только
разрешённые поля. После проверки оно добавляет необходимые `LEFT JOIN`, `CASE`
и SQL-выражение представления. Callback вызывается во время компиляции сразу
для пакета типов, а не для каждой строки результата, поэтому приложение может
кешировать планы по GUID и поколению метаданных. CLI использует для этого
ограниченный кеш Moka.

> [!NOTE]
> Здесь ABI означает типизированный контракт между ядром и приложением. Сейчас
> это публичный Rust API; стабильного `extern "C"` ABI для подключения из других
> языков в проекте пока нет. `ObjectId::as_bytes()` и `AttributeId::as_bytes()`
> позволяют построить такой адаптер без передачи строковых имён.

## Лицензия

[GNU General Public License v3.0 only](LICENSE).
