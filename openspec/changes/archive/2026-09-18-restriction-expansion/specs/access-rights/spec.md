## ADDED Requirements

### Requirement: Expand a restriction text
The library SHALL expand a restriction text of a role into a condition:
a template call `#Имя(аргументы)` SHALL be replaced by the body of the
role's template of that name with `#Параметр(N)` and the named
parameters of its signature replaced by the arguments, recursively up to
a bounded depth; `#ИмяТекущейТаблицы` SHALL stand for the name of the
restricted table and `#ИмяТекущегоПраваДоступа` for the Russian name of
the right; `#Если <выражение> #Тогда`, `#ИначеЕсли`, `#Иначе` and
`#КонецЕсли` SHALL keep the branch whose expression holds, where an
expression combines `&Параметр` session values, string literals, the
two names above, `Истина`/`Ложь`, `+`, `=`, `<>`, `Не`, `И`, `Или`,
parentheses and `СтрСодержит`; comments `//` SHALL be dropped. A
session parameter the expression names without a value SHALL be an
error naming the parameter. A result that is a labelled message —
`Ошибка: …`, `НеверноеПраво: …` — SHALL be an error carrying the message.
The expanded text SHALL be the platform's form
`[ТекущаяТаблица [КАК <псевдоним>]] [ГДЕ] <условие>`, returned as the
condition with its alias, and a join written before `ГДЕ` SHALL be an
error.

#### Scenario: Universal restriction branch
- **WHEN** the condition of `ЧтениеЭлектронныхДокументов` is expanded
  with `ОграничениеДоступаНаУровнеЗаписейУниверсально = Истина`,
  `СпискиСОтключеннымОграничениемЧтения = "Все"` and
  `ВерсииШаблоновОграниченияДоступа = ",ДляОбъекта9,"`
- **THEN** the result is the condition `ИСТИНА` from the template
  `ДляОбъекта`

#### Scenario: Outdated template
- **WHEN** the same condition is expanded with
  `ВерсииШаблоновОграниченияДоступа = ""`
- **THEN** expansion fails with the message starting `Ошибка: Требуется
  обновить шаблон`

#### Scenario: Missing session parameter
- **WHEN** the same condition is expanded without
  `СпискиСОтключеннымОграничениемЧтения`
- **THEN** expansion fails naming that parameter

### Requirement: Combine the access of a user's roles
`read_access` SHALL answer, for a set of roles, one object and one
right: `Denied` when no role grants the right; `Unrestricted` when a
role grants it without a restriction, whatever the other roles restrict;
otherwise `Restricted` with the expanded restriction of every granting
role. `Access::condition` SHALL join the restrictions with `ИЛИ` under
one alias, or fail when the aliases differ.

#### Scenario: One role restricts, another grants freely
- **WHEN** a user holds a role restricting `Чтение` of a catalog and a
  role granting it without restriction
- **THEN** the access is `Unrestricted`

#### Scenario: Two restricting roles
- **WHEN** both roles restrict the right with conditions `А` and `Б`
- **THEN** the access is `Restricted` and its condition is `(А) ИЛИ (Б)`

#### Scenario: No role grants
- **WHEN** no role of the user lists the right as granted
- **THEN** the access is `Denied`
