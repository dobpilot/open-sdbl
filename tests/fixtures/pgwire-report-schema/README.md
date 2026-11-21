Designer 8.3.27.2342 loaded the existing empty report `ЗамерОтчета` into
a disposable configuration with two data composition templates. The selected
main schema was `ВыбраннаяСхема`, despite also declaring a template named
`ОсновнаяСхемаКомпоновкиДанных`.

The raw-DEFLATE resources come from `/DumpCfg`. The report XML comes from a
separate `/DumpConfigToFiles` of that configuration. The zero-based slot 5 of
report class 19/18 holds the exact GUID of `ВыбраннаяСхема`. The existing empty
report fixture has the same profile and a nil GUID in that slot.
