# Нативное сопоставление заимствованного элемента стиля

1С 8.3.27.2342, 2026-09-30. Только `/tmp/sdbl-095id-file` и SQL-копия `sdbl_measure_095style` изменялись. `buh` использовалась исключительно для read-only `ibcmd config save --extension=ФормированиеТестовыхДанных --user=admin` и XML export. Исходный CFE сохранён `/tmp/sdbl095-buh-native.cfe`; SHA-256 рядом.

Исходный CFE установлен бинарным `config load`, затем `config apply`; XML не перекомпилировался. Платформа сообщила поколение `efc85a95fd12b00737d88782e29d9a975342452d`, совпадающее с исходным buh CAS. Legacy descriptor `d44efef3-da72-4d62-a1a2-3fc5a55e23b8` находится в `../legacy-buh/`, native XML: StyleItem.НадписьПоясняющаяГиперссылку, Color, Adopted, header v1 без target-поля.

Два элемента стиля основной одноразовой конфигурации создаются с контролируемыми UUID: A=`55914c59-1de8-46e0-9b41-ce829c8b84bf`, B=`68c21e2d-b4a7-4a9c-a59a-12b4de247827`. `base-probe-a.xml` и `base-probe-b.xml` — нативная нормализованная XML-выгрузка перед переименованиями; Name меняется, UUID сохраняется.

| Артефакт | Состояние базы | Нативная проверка |
| --- | --- | --- |
| legacy-original-missing | имени нет | 56 ошибок, Object not found для целевого StyleItem |
| legacy-name-uuid-a | A носит точное имя | 55 ошибок, ошибки целевого объекта нет |
| legacy-renamed-uuid-a | A переименован, UUID прежний | 56 ошибок, целевой объект не найден |
| legacy-name-uuid-b | B с другим UUID получает точное имя | 55 ошибок, ошибки целевого объекта нет |
| case-name-uuid-b | B носит имя в нижнем регистре | найден, но контролируемое Name не совпадает; ошибка целевого объекта |

Остальные 55 объектов исходного расширения в одноразовой конфигурации отсутствуют. Измерена именно диагностика сопоставления данного объекта в `ПроверитьВозможностьПрименения()`, а не успешное применение всего исходного расширения. В check-скрипте сохраняются все диагностики и отдельно число ошибок целевого StyleItem.

Также создан минимальный Probe095ID, compat8.3.17, local UUID `e77056e7-a80e-4260-8aa0-c70022c4a7f9`. Его native stored header v3 с nil target сохранён `native-v3-nil.serialized` (из read-only SQL-копии). initial/base-renamed/different-uuid-same-name дают 0/1/0 ошибок при сохранении UUID A, затем подстановке UUID B под исходное имя. Source KeepMapping=true. После native XML изменения KeepMapping=false и имени на целевое русское имя, mapping-false-name-uuid-b даёт 0 ошибок. Case probe даёт тот же Name mismatch и для этого v3-nil расширения.

При регистровом изменении возникает не Object not found, а `Value of Name property ... with value check enabled does not match ...`. Поэтому принятие только по folded equality недостаточно. Точное совпадение имени является безопасным измеренным подмножеством; отдельные правила отключения контроля Name не измерялись.

## Непригодная проба getter

`Новый ОбъектМетаданныхКонфигурация(ДвоичныеДанные CFE)` возвращает Undefined для ОбъектРасширяемойКонфигурации и у legacy style, и у canary НекорректныйКонтрагентЦветФонаВСписке, чья нативная XML имеет explicit target `c6b23cb2-5d67-4bf0-bfb1-fadfd0d1f522`. Поэтому `buh-compiled-metadata-canary.platform.txt` не является доказательством отсутствия native mapping. Достоверные выводы основаны на controlled UUID/name изменениях и нативной проверке применимости.

Никакие target GUID по имени не добавлены в open-sdbl. Другие классы и отключение проверки Name этим замером не покрыты.

Текстовые артефакты нормализованы: UTF-8 без BOM, LF, без пробелов в конце строк.

После KeepMapping=false и case mismatch UUID B переименован в ProbeB_R, UUID A получил точное русское имя. `mapping-false-different-uuid-a.platform.txt`: CheckCanApply минимального v3-nil — 0, target errors original v1 — 0 (55 других ошибок). Это подтверждает переключение на другой UUID по точному имени и при выключенном KeepMapping. Native поколения: original buh extension `efc85a95fd12b00737d88782e29d9a975342452d`, minimal extension mapping false `3b5d3de7a153f3cca3d891bd08bfe58b9c45dacd`, base case-change `c4b7b53b8b5fbf4ab23986c78afcd15900000000`, base final UUID A `951e7322791cf249a633ed82c8b932d600000000`.
