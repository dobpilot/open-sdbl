# Design

UserData.interface_language: Option<Guid> carries Some(non-nil), Some(nil), or None. InfoBaseUser already owns UserData; async read_users and blocking ensure_users share it. Only the measured 38-field layout with zero separation bindings, one optional string and validated suffix is recognized; existing separation validation supplies structural checks. Other historical layouts remain unknown for language. Language corruption never changes existing identity/name/role decoding. No locale lookup, host fallback, metadata acquisition or new SQL is added.

Evidence: open-bsl current-language-session-audit/native-user-records-1017 paired native DT and four raw own-user records. Tests retain original encrypted fixtures and provenance, never print raw records or password fields. Existing Debug exposes only decoded non-secret fields. Comments and new rustdoc are Russian.
