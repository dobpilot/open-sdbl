# Design

Add compile_system_settings_load with the same keys and CommonSettingsSeparation states as compile_common_settings_load. Both call a private compiler with a fixed internal storage choice; callers cannot supply arbitrary tables. All schema and scope validation remains shared. Preserve common-settings diagnostics and generated SQL. System settings validate their own acquired table and types, without borrowing columns from CommonSettings. PostgreSQL only; other backends fail. No acquisition changes, decoder duplication, writes or principal inference.

Native 1C 8.3.27.2342 controls prove root/A/B string isolation and numeric conditional namespaces: true/0 is 20, true/101 is 21, true/202 is 22, false/0 is 23. Reuse current container decoder, including storage Structure payloads. Unknown profiles fail explicitly.
