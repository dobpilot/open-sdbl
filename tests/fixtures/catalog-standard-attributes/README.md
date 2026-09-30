# Стандартные реквизиты справочников

1С 8.3.27.2342. `unf-native.tsv` извлечён из нативной XML-выгрузки `/tmp/open-bsl-unf-query-schema-xml/Catalogs`, сопоставленной по UUID с read-only `tests/fixtures/unf/config.pack`: UUID, Name, CodeType, CodeLength, DescriptionLength. Все 296 записей (290 String, 6 Number) совпали с позициями 17,18,19 списка класса 57 Config. XML из иной конфигурации по общему UUID не использовалась как эталон.

`versions.deflate` — один Config ресурс `4bb0f7c3-62f3-4352-9bc8-e243dd18fe4a`, считанный SELECT из UNF 2026-09-30; `versions.native.xml` — его нативная XML: Number,9,0. Никаких записей в рабочие базы. Это метаданные, не измерение начального значения BSL. Иерархия и владельцы этим API не декодируются.

`unf-presentation-native.tsv` — отдельная UUID/Name/DefaultPresentation матрица из той же нативной XML: 26 AsCode и 270 AsDescription. Позиция 20 списка класса 57 подтверждена без расхождений; настройка не выводится из CodeType или длин.
