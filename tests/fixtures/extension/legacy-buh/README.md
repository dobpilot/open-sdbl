# Нативная форма legacy-заголовков buh

1С 8.3.27.2342; read-only `ibcmd config export --extension=ФормированиеТестовыхДанных --user=admin`. Существующая база не изменялась. `native-objects.tsv` — UUID/Name/ObjectBelonging и путь из нативных XML; три XML сохранены целиком. `.serialized` — распакованные bare-GUID ресурсы ConfigCAS, приобретённые read-only сеансом. Все 37 строк до исправления не имели собственного заголовка; каталог расширения был известен. Версия заголовка 1, identity `{0,0,guid}`, explicit Adopted. Пары в хвосте относятся к изменениям свойств и не дают явной ссылки на базовый объект.

Для текстового хранения убраны UTF-8 BOM и CRLF; записи и XML-свойства не изменены.

## Неизвестная связь заимствования

UUID legacy `Catalog.Валюты` — `ba427672-e557-4a0c-97f4-dce7c0f6b2fb`; UUID основной таблицы из read-only metadata snapshot — `225d7842-f8f4-4d0e-9173-7b0c9608e4af`. Они различны; GUID основной таблицы не встречается в serialized описании расширения. Native XML содержит `ObjectBelonging=Adopted`, но не target GUID. Root XML содержит `KeepMappingToExtendedConfigurationObjectsByIDs=true` и `ConfigurationExtensionCompatibilityMode=Version8_3_17`: поэтому отсутствие target в header не доказывает adoption-by-name. Место отдельного mapping не установлено. `extends=None` сохраняет эту неизвестность; сопоставление по имени не выполняется.
