# Native runtime extension properties (1C 8.3.27.2342)

On 2026-09-30, native `ibcmd infobase create --dbms=PostgreSQL --locale=ru_RU --create-database` created disposable `sdbl_measure_095b`. Native `ibcmd extension create --name=Probe095 --name-prefix=Probe095_ --purpose=add-on` created the extension. No properties or configuration were changed in demo, buh or unf.

Each `*.info.txt` is native `ibcmd extension info`; its paired `*.row.tsv` is read-only SQL `SELECT _extname,_extensionscope,encode(_extensionzippedinfo,'hex') FROM _extensionsinfo`. Updates are retained as `*.update.txt`. The password prompt is omitted and line endings/trailing terminal spaces are normalized; property lines are retained unchanged and no password is captured.

The first native generation was `48d4a1cd06b0bd4bb66129b671f28ad5e46bb642`, HashSum `SNShzQawvUu2YSm2cfKK1eRrtkI=`. After importing Version=0.9.5 in the extension XML, the versioned generation is `2fd0d9e318ca033755df9a4157bfa725af0f4ac0`, HashSum `L9DZ4xjKAzdV35pBV7+nJa8PSsA=`. Changing KeepMappingToExtendedConfigurationObjectsByIDs generated `9bc53fc9391cc761baecb2e9573837eac9a70d5b`, HashSum `m8U/yTkcx2G67LLpVzg36smnDVs=`. Every native hash matches standard Base64 of the record's twenty-byte root key.

In each record, byte 35 is SafeMode: 0x82 true, 0x81 false. Byte 36 is empty profile 0x81 or a counted ASCII/UTF-16 profile string. The eight-byte counter before SafeMode changes on updates and is not a permission flag. Profile bytes must be skipped when decoding synonym/version. Native sets safe-mode=yes when assigning an ASCII or Unicode profile; the profile name remains a separate field.

Native scope infobase maps to SQL code 1. Attempting `--scope=data-separation` failed: `Cannot set the configuration extension scope to DataSeparation in the infobase that does not use data separation`. No DataSeparation numeric mapping was inferred.

Byte 24 is native unsafe-action protection: its generated 0xa1 variant is retained as `unsafe-protection-disabled.*`, produced by native `--unsafe-action-protection=no`. `legacy-a1-profile.*` is an earlier parser probe which temporarily changed that byte directly in the DISPOSABLE SQL row; it was restored in a finally block. This synthetic probe is distinguished from the later native-generated observation and is not the sole evidence for supporting 0xa1.

Record completeness is required: header, counted synonym, absent/present version, final Boolean flags and terminator. Unknown/truncated variants retain None. The production parser does not interpret counters or unmeasured scope codes.

## UUID и известное отсутствие версии

`identity-empty-version` снят на новой PostgreSQL базе `sdbl_measure_095id`:
`ibcmd infobase create --locale=ru_RU --create-database`,
`extension create --name=Probe095ID --name-prefix=Probe095ID_ --purpose=add-on`.
SQL читает `_ExtName`, hex `_IDRRef`, hex `_ExtensionZippedInfo`.
Нативный `extension info` возвращает пустую `version`.
Затем `ibcmd infobase dump` и `infobase create --restore` переносят базу
в одноразовую FILE-базу. Нативный BSL выводит для каждого элемента
`РасширенияКонфигурации.Получить()` имя, строку свойства
`УникальныйИдентификатор` и свойство `Версия`.
Результат сохранён рядом; GUID совпадает с перестановкой физического
`_IDRRef` `[12..16] + [10..12] + [8..10] + [0..8]`.
Новая `runtime_version` отличается от старой частичной `version`:
измеренный маркер отсутствия даёт `Some("")`, неизвестная запись — `None`.
