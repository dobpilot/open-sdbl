# Native data composition template

Designer 8.3.27.2342 loaded the neighboring `data-processor-header2/control.cf`
into a disposable file base and exported its XML. For the test, the layout
`Обработка.ПомощникСозданияАвтономногоРабочегоМеста.Макеты.ИнструкцияПоНастройкеАРМ`
(UUID `59aadc03-adbc-481f-89bc-d7f391789522`) was changed to
`DataCompositionSchema` with a minimal schema and then loaded back with
`/LoadConfigFromFiles`.

The `*.deflate` files are the exact header and body from the subsequent
`/DumpCfg`. The XML is the same layout from a separate
`/DumpConfigToFiles` run. The body has a zero marker, one settings variant,
two 64-bit little-endian lengths, and three UTF-8 XML documents. The first
is the original runtime schema with the variant reference, the second
holds the settings, and the third is an auxiliary schema.
