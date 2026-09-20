## MODIFIED Requirements

### Requirement: Expand a restriction text
The library SHALL expand a restriction text of a role into a condition:
a template call `#Имя("аргумент", …)` SHALL be replaced by the body of
the role's template of that name, with every argument unquoted — two
double quotes standing for one inside it — and with `#Параметр(N)` and
the named parameters of the signature replaced by the argument of that
position, recursively up to a bounded depth. A `#Имя` naming a template
SHALL be a call whether or not a parenthesis follows it; without one it
passes no argument.

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
`[ТекущаяТаблица [КАК <псевдоним>] [ИЗ <таблица> [КАК <псевдоним>]] [<соединения>]] [ГДЕ] <условие>`,
returned as the condition with its alias and the text of its join
clauses, where the source description SHALL name the restricted table —
by its query name or as `ТекущаяТаблица` — and SHALL give the alias the
condition reads. A source description naming another table, and a second
source written with a comma, SHALL be an error naming what was found.
The join clauses SHALL be answered as they were written, for the
compiler to read, and [`ExpandedRestriction::text`] SHALL write them
between the table and `ГДЕ`.

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

#### Scenario: A call without the parenthesis
- **WHEN** the restriction of a role reads `#ЧтениеШаблоновПроцессов`
  and the role carries a template of that name
- **THEN** the body of that template stands in its place, and no `#`
  survives the expansion

#### Scenario: A restriction that joins
- **WHEN** the text reads
  `ТекущаяТаблица ИЗ #ТекущаяТаблица КАК Т ЛЕВОЕ СОЕДИНЕНИЕ РегистрСведений.Д КАК Д ПО Т.Ссылка = Д.Объект ГДЕ Д.Поле`
- **THEN** the join clause is answered beside the condition `Д.Поле`,
  and the text written for the compiler carries both

### Requirement: Combine the access of a user's roles
`read_access` SHALL answer, for a set of roles, one object and one
right: `Denied` when no role grants the right; `Unrestricted` when a
role grants it without a restriction, whatever the other roles restrict;
otherwise `Restricted` with the expanded restriction of every granting
role. `Access::condition` SHALL join the restrictions with `ИЛИ` under
one alias, or fail when the aliases differ or when more than one of the
restrictions joins other tables, which cannot be merged.

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

#### Scenario: Two restrictions that join
- **WHEN** two of the granting roles restrict the right with texts that
  join other tables
- **THEN** `Access::condition` fails, saying they cannot be merged
