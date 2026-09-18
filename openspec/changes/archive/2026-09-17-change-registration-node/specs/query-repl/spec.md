## ADDED Requirements

### Requirement: Expose the change-registration fields
The change-registration table of an object SHALL expose `Узел`/`Node` —
the exchange-plan node the change is registered for, with the reference
behaviour of any reference field — and `НомерСообщения`/`MessageNo` as
standard fields beside the registered object's key.

#### Scenario: Changes of one node
- **WHEN** `ВЫБРАТЬ И.Ссылка ИЗ Справочник.Номенклатура.Изменения КАК И ГДЕ И.Узел = &Узел И И.НомерСообщения ЕСТЬ NULL` is compiled
- **THEN** the SQL compares the node columns of the change table with
  the parameter and tests `_MessageNo` for NULL
