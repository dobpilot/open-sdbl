# Аудит запросов open-bsl к SDK 0.9.6

Проверены восемь `docs/plans/open-sdbl-*-request.md` потребителя на ветке `open-sdbl-v9`, после коммитов `5f23f57`, `8058cf2`, `e3ffa13` и `d08a4cc`. Версия остаётся 0.9.6. Этот аудит не означает завершения ORM или запуска исходных сеансов demo/buh/unf.

| Запрос | Реализованный контракт SDK |
| --- | --- |
| configuration-acquisition | `DatabaseSession::configuration`, `AcquiredConfiguration`, `ConfigResource`, отдельные `AcquiredExtension`, общий pipeline, сборка частей, независимые потолки Config/ConfigCAS; `InfoBaseUser::can_authenticate` |
| blocking-session | `BlockingDatabaseSession::{connect,configuration,configuration_selected,metadata,query,query_each,close,dialect,is_closed,is_dead,ensure_users,ensure_rights}`, `Credentials::new`, `EnvironmentSecret::from_option`; общая асинхронная реализация; типизированные SessionClosed/AsyncContext |
| catalog-selection-metadata | `ConfigDescriptor::catalog_owners: Option<Vec<Guid>>`, `attribute_indexing: Option<AttributeIndexing>` с Disabled/Enabled/WithAdditionalOrder; сопоставление native XML/Config в `runtime_metadata_096` |
| field-rights-fix | `check_user_field_access`: измеренное программное чтение полей каталога, запрет неизвестной формы и явных отказов; тесты прав и нативные роли `access/native-096`; права/RLS/разделители не отключаются |
| document-object-metadata | `document_number`, `document_attributes`, `document_tabular_sections`, `declared_types`, `fill_from_filling_value`, `filling_value`, `defined_type_id`, `object_reference_type`; полные объединения и раскрытие именованных типов, неизвестное не превращается в пустое |
| document-tabular-section-fields | `tabular_section_attributes: Option<Vec<Guid>>`; 522 части/4519 полей УНФ и минимальный native документ с независимыми частями 2/2/0 |
| document-register-associations | `document_registers: Option<Vec<Guid>>`; 125 документов/1355 связей УНФ, включая 51 у ЗаказПокупателя, пустые и смешанные коллекции |
| document-numbering-metadata | `document_numbering: Option<DocumentNumberingSettings>` с пятью измеренными периодичностями и `numerator: Option<Guid>`; внешний None неизвестен, внутренний None означает проверенно пустую ссылку; 125 документов и семь минимальных native документов |

Расширения имеют измеренные optional-настройки версии, безопасного режима, профиля, hash, области и распределённой базы, корневой каталог и полный optional-список заголовков со связями Own/Adopted/extends. Неизвестные формы сохраняют неизвестность. BSL-слоты, эффективный граф кода и исполнение остаются у потребителя.

В повторном аудите найден один невыполненный пункт исходного blocking-session запроса: документация ошибочно рекомендовала старый `CONFIG_FINGERPRINT` для всей приобретённой конфигурации. Выбран явно разрешённый запросом вариант — честный узкий контракт. Оба fingerprint-запроса сохраняют прежний SQL и фильтр: bare GUID либо `.1c`/`.9`/`.7` в Config, без прочих ресурсов и без ConfigCAS/_ExtensionsInfo/_ExtensionsRestruct. Равенство не доказывает неизменность полного/выборочного приобретения или снимка с расширениями. Это зафиксировано отдельной дельтой `clarify-configuration-fingerprint-scope` и тестами `configuration_fingerprint_scope`. Также исправлен устаревший rustdoc `check_user_field_access`, всё ещё описывавший отказ для всех стандартных полей и обязательное отдельное разрешение любого реквизита. Это механическая синхронизация с уже принятым требованием `Map verified catalog programmatic reads` и существующей реализацией; новая дельта или изменение runtime-прав для неё не нужны.

## Реальные ограничения

- READ COMMITTED не даёт атомарного состояния всех acquisition-запросов; полного fingerprint поверхности нет.
- Live buh проверка прежнего исправления прав дошла до авторизации полей Ссылка/ИНН и RLS, затем остановилась на отсутствующем параметре разделителя. Это не успешное SQL-чтение и не полный исходный сеанс.
- Неизмеренные формы метаданных/расширений и несопоставимые права остаются неизвестными или отклоняются; SDK не выдаёт их за поддержанные.
- Нет выдачи/резервирования номера, SQL-записи документов или подтверждённого runtime-поведения общего нумератора. CheckUnique и дополнительные настройки не публикуются без отдельного измерения.
- Минимальные живые проверки выполнены на PostgreSQL; живой SQL Server и Windows не проверялись. Синхронная отмена извне другого потока не добавлена; существующие Limits и callback-отказы сохраняют общий путь.

Новые реальные дефекты вне этих запросов следует оформлять отдельно. Уже выполненные запросы сами по себе не требуют искусственных изменений API.
