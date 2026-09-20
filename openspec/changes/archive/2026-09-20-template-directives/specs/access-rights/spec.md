## MODIFIED Requirements

### Requirement: Expand a restriction text
The library SHALL expand a restriction text of a role into a condition:
a template call `#Имя("аргумент", …)` SHALL be replaced by the body of
the role's template of that name, with every argument unquoted — two
double quotes standing for one inside it — and with `#Параметр(N)` and
the named parameters of the signature replaced by the argument of that
position, recursively up to a bounded depth.

`#ТекущаяТаблица` SHALL stand for the name of the restricted table and
`#ИмяТекущейТаблицы` for that name as a string value, in quotes;
`#ИмяТекущегоПраваДоступа` SHALL stand for the Russian name of the
right; `##` SHALL stand for one `#` and SHALL be read as text wherever a
directive, a call or a parameter is looked for.

`#Если <выражение> #Тогда`, `#ИначеЕсли`, `#Иначе` and
`#КонецЕсли` SHALL keep the branch whose expression holds, where an
expression combines `&Параметр` session values, string literals, the
names above, `Истина`/`Ложь`, `+`, `=`, `<>`, `Не`, `И`, `Или`,
parentheses and `СтрСодержит`; comments `//` SHALL be dropped. A
session parameter the expression names without a value SHALL be an
error naming the parameter. A result that is a labelled message —
`Ошибка: …`, `НеверноеПраво: …` — SHALL be an error carrying the message.

The expanded text SHALL be the platform's form
`[ТекущаяТаблица [КАК <псевдоним>] [ИЗ <таблица> [КАК <псевдоним>]]] [ГДЕ] <условие>`,
returned as the condition with its alias, where the source description
SHALL name the restricted table — by its query name or as
`ТекущаяТаблица` — and SHALL give the alias the condition reads. A source
description naming another table, and anything else written before `ГДЕ`
such as a join or a second source, SHALL be an error naming what was
found.

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

#### Scenario: The source description of the full form
- **WHEN** a template expands to
  `ТекущаяТаблица ИЗ #ТекущаяТаблица КАК ТекущаяТаблица ГДЕ <условие>`
  for `Справочник.Файлы`
- **THEN** the condition is `<условие>` with the alias
  `ТекущаяТаблица`, and the table name stands where the directive was

#### Scenario: A source that is another table
- **WHEN** the description reads `ИЗ Справочник.Другой КАК Т`
- **THEN** expansion fails naming that table

#### Scenario: A parameter by number
- **WHEN** the body `Итого = #Параметр(1)` is called as `#Шаблон("10")`
- **THEN** the text is `Итого = 10`

#### Scenario: An argument carrying quotes
- **WHEN** the body `ВидДокумента = #ВидДокумента` of `Шаблон1(ВидДокумента)`
  is called with the argument `"""Накладная"""`
- **THEN** the text is `ВидДокумента = "Накладная"`

#### Scenario: The escaped number sign
- **WHEN** a body reads `#Параметр(1) ## #Параметр(2)`
- **THEN** one `#` stands between the arguments, and `##Если` is text,
  not a directive
