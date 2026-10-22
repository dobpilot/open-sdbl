# Recognize an extension role with no rights subobject

The BUH extension role `ДемоПроф_ОбщиеПрава` has a complete native Role descriptor but no `<guid>.0` entry in its extension index. The current fallback treats that representation as unread rights and prevents a complete selected-user decision.

Recognize only the measured complete descriptor form that explicitly carries no rights subobject. Return an empty `RoleRights` for that form when the index declares no `.0`. Preserve refusal for unavailable descriptors, unknown forms, promised but unavailable rights resources and malformed stored resources. Existing rights resources continue through the existing decoder. Public APIs and versions remain unchanged.
