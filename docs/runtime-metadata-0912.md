# Приобретённые метаданные 0.9.12

SDK приобретает описатели и не исполняет BSL. Поля находятся в `metadata::ConfigDescriptor`; `Option` отличает неизвестное от проверенного значения или пустого набора.

Функциональные опции:

- `functional_option_location_guid: Option<Guid>` сохраняет исходную ссылку.
- `functional_option_location: Option<FunctionalOptionLocation>`: `Constant(Guid)`, `CatalogAttribute { owner: Guid, attribute: Guid }`, `InformationRegisterResource { owner: Guid, resource: Guid }`, `Absent`. Ссылка разрешается только по приобретённым владельцам.
- `functional_option_privileged_get_mode: Option<bool>` хранит явный флаг; неизвестное не означает false.
- `functional_option_parameter_use: Option<Vec<Guid>>` хранит полный упорядоченный Use параметра.
- `configuration_functional_option_parameters: Option<Vec<Guid>>` хранит полный корневой состав параметров. `Some(vec![])` означает проверенный пустой состав. Отсутствующая ссылка, неизвестный Use или неполная коллекция обнуляют полноту.
- `catalog_attribute_use: Option<CatalogAttributeUse>`: `ForItem`, `ForFolder`, `ForFolderAndItem`.
- `information_register_dimensions: Option<Vec<Guid>>` и `information_register_resources: Option<Vec<Guid>>` сохраняют полный упорядоченный состав с проверенным счётчиком, отдельно от field_purpose. Отсутствующий член, неверный счётчик, дубликат или неизвестный тип дают `None` целиком; проверенная пустота — `Some(vec![])`.
- `information_register_write_mode: Option<InformationRegisterWriteMode>`: `Independent`, `RecorderSubordinate`.
- `information_register_periodicity: Option<InformationRegisterPeriodicity>`: `Nonperiodical`, `Day`. Другие коды остаются неизвестными. Описатель периодического/подчинённого регистра не является обещанием поддержки исполнения опции этим потребителем.

Общие реквизиты:

- `common_attribute_separation: Option<CommonAttributeSeparation>`: `DontUse`, `Separate`, независимо от наличия `separation: Option<DataSeparationSettings>`.
- `common_attribute_usage: Option<CommonAttributeUsage>` содержит исходные явные настройки Config. `auto_use` остаётся двухзначным `CommonAttributeUse`; `content[].usage` теперь `CommonAttributeContentUse::{Auto, Use, DontUse}`. Этот список не является полным runtime Состав.
- `configuration_common_attribute_candidates: Option<Vec<CommonAttributeCandidate>>` хранит проверенный корневой состав возможных владельцев. Служебные признаки `constant`, `separated_only` и `unseparated_only` отражают измеренную применимость.
- `common_attribute_content: Option<Vec<CommonAttributeRuntimeContentEntry>>` содержит полный приобретённый runtime Состав с `object: Guid`, `usage: CommonAttributeContentUse`, `conditional_separation: Option<CommonAttributeConditionalSeparation>`. Нативный порядок — возрастание GUID; порядок явных XML Items его не меняет.
- `CommonAttributeConditionalSeparation::{Undefined, Constant(Guid)}` отличает подтверждённое отсутствие от неизвестного. Ненулевая ссылка разрешается только в приобретённую Boolean-константу; другие формы остаются `None`, исходный GUID сохраняется в явных `CommonAttributeContentEntry`.

Полный Состав строится лишь из полностью проверенного корневого inventory и приобретённых владельцев. Separate включает константы и регламентные задания и исключает журналы документов/последовательности; DontUse включает последние и исключает константы. Остальные измеренные persistent классы представлены в парном corpus. Перечисления, отчёты, обработки и нумераторы не входят. Непустые внешние источники, неподдержанный root layout и отсутствующие применимые владельцы дают `None` для всего списка. Исходные явные настройки сохраняются отдельно. В контрольном DontUse исходный XML содержал Constant.Use, но платформа удалила эту запись при сохранении; SDK проверяет итоговую независимую пару CF/XML.

## Доказательства и проверки

`tests/fixtures/runtime-options-0912/` хранит независимые native CF и обратные XML, исходные compressed ресурсы, распакованный Config, BSL probe и runtime receipts. `provenance.json` содержит SHA256. Платформа 8.3.27.2342, собственные свежие IB в `/tmp`; рабочие БД не менялись.

Контроли `native`, `catalog-native`, `register-native` подтверждают три источника, флаги и 0/1/2 параметра. `condition`, `order`, `order-reversed` подтверждают ненулевое условное разделение и независимость порядка от Items. `separate`/`disabled` подтверждают полный состав 14/15 владельцев и исключённые классы. `settings` подтверждает три catalog Use, независимый непериодический/дневной и подчинённый непериодический регистр, две ссылки одного параметра, явный Auto и полный состав 16 владельцев.

`tests/runtime_options_0912.rs` сравнивает полный content с native oracle; DB acquisition test удаляет все исходные ресурсы после разбора и проверяет сохранение приобретённого состава и Use. Неподдержанные флаги, неполные списки, неизвестный корневой состав и отсутствующие владельцы проверяются отдельно.

Парные `scheduled-separate`/`scheduled-disabled` подтверждают применимость регламентных заданий только при Separate. Это приобретение метаданных, без выполнения заданий.

Фактическая read-only acquisition demo после исправления: `configuration_common_attribute_candidates = Some(1715)`, `common_attribute_content = Some` для всех 13 общих реквизитов, неизвестных составов нет. Корень содержит 157 регламентных заданий. Проверка не запускала BSL; это доказательство полноты приобретённых метаданных, а не успешного запуска приложения.
