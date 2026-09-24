# Хранилище расширений демо-базы

Строки `ConfigCAS` и `_ExtensionsInfo` базы `demo` (PostgreSQL, платформа
8.3.27) с двумя расширениями: `Расширение1` применяется, `Расширение2` — нет.

| Файл | Содержимое |
| --- | --- |
| `<ключ>.deflate` | строка `ConfigCAS` с именем `<ключ>`, байты как их хранит база (сырой DEFLATE) |
| `extensions.tsv` | `_IDRRef`, `_ExtensionOrder`, `_ExtName`, `_ExtensionScope`, `_UsedInDistributedInfoBase` (0/1), `_ExtensionZippedInfo` — всё в hex, где это байты |

Ключевые ресурсы `Расширение1`:

| Ключ | Ресурс |
| --- | --- |
| `89c0c813…` | корневой индекс расширения |
| `8f815a90…` | корень конфигурации `182ffc1d-7701-4067-a547-c7b78532e40d` |
| `33b58e11…` | заимствованный справочник `СтавкиНДС` (`e1945025-…`), расширяет `6a386de9-…` — `Reference14574` базы |
| `3a5bbff8…` | собственный справочник `Расш1_Справочник1` (`b7608864-…`) |

Снято SELECT-запросами:

```sql
SELECT encode(_idrref,'hex'), _extensionorder::int, _extname::text,
       _extensionscope::int, _usedindistributedinfobase::int,
       encode(_extensionzippedinfo,'hex')
FROM _extensionsinfo ORDER BY _extensionorder;
SELECT binarydata FROM configcas WHERE rtrim(filename::text) = '<ключ>' ORDER BY partno;
```

Рядом, `../extension_info_versioned.bin`, — `_ExtensionZippedInfo`
расширения `ПутеводительПоДемоБазеПроф` базы `buh`: запись с версией `2.4`.
