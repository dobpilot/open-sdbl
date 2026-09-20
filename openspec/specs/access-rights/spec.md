# access-rights Specification

## Purpose

Define how the library reads who may see what in an information base: the
roles a configuration declares with the rights and row-level restriction
texts they carry, the users of the base with their roles, and the
expansion of a restriction into a condition the compiler applies.

## Requirements

### Requirement: Decode the rights resource of a role
The library SHALL decode a role's `<guid>.0` Config resource into
`RoleRights`: the header flags `setForNewObjects`,
`setForAttributesByDefault` and `independentRightsOfChildObjects`; one
`ObjectRights` per metadata object or object member the resource lists,
each right with whether it is granted (`1`) or refused (`-1`) and its
restriction conditions with their field identifiers; and the restriction
templates with their signature and body. The resource SHALL be decoded
by shape, whatever its file name, so a `ConfigCas` resource decodes the
same way. A resource of another format version SHALL be refused with a
`MetadataError`.

#### Scenario: A read-only role with a restriction
- **WHEN** the rights resource of `ЧтениеЭлектронныхДокументов` (БП 3.0)
  is decoded
- **THEN** the first object grants `Read`, `View` and `InputByString`,
  `Read` carries the condition starting with
  `#Если &ОграничениеДоступаНаУровнеЗаписейУниверсально #Тогда`, and the
  four templates `ДляОбъекта`, `ДляРегистра`, `ПоЗначениям`,
  `ПоЗначениямРасширенный` are listed

#### Scenario: Refused rights
- **WHEN** a rights list records `-1` for a right
- **THEN** that right is present with `allowed == false`

### Requirement: Name the standard rights
`Right` SHALL name the standard right identifiers by their platform
names — `Read`, `Insert`, `Update`, `Delete`, `View`, `Edit`,
`InputByString`, the interactive and data-history rights, the document
posting rights, `TotalsControl`, `Use`, the business-process and task
rights, the session-parameter rights `Get` and `Set`, and the
configuration rights from `Administration` to `Output` — with their
Russian synonyms where the platform has one, and SHALL carry any other
identifier as `Right::Other(guid)`.

#### Scenario: The reading right
- **WHEN** the identifier `1c87578f-9e09-4ec0-a991-5629c87b1588` is named
- **THEN** it is `Right::Read`, spelled `Чтение`

#### Scenario: An unknown identifier
- **WHEN** an identifier the table does not list is named
- **THEN** it is `Right::Other` with that identifier

### Requirement: Catalog the roles of a configuration
The library SHALL list the roles a configuration declares from the roles
collection of the configuration root and name them from their bare-GUID
descriptors: `RoleCatalog` SHALL answer every role with its identifier,
name and synonym, and SHALL find a role by name or by identifier.

#### Scenario: Roles named from descriptors
- **WHEN** the root resource lists two role identifiers and the
  descriptors of both are parsed
- **THEN** the catalog names both and finds `АдминистраторСистемы` by
  name and by identifier

### Requirement: Decode the information-base users
The library SHALL decode the `Data` column of `v8users` — a blob whose
first byte is the length of the XOR key that follows it, the rest being
the brace-serialized user record XORed with that key, with a UTF-8
byte-order mark in front — into `UserData`: the user identifier, the
name, the full name and the role identifiers of the `{N, <guid>…}`
block. The password hashes the record carries SHALL NOT be exposed.
`InfoBaseUser` SHALL combine the row of `v8users` — name, description,
operating-system login, e-mail (empty on a platform whose table has no
such column), the show-in-list, standard-authentication and
administrative flags — with the decoded data, and SHALL name the roles
through a `RoleCatalog`.

#### Scenario: A user with three roles
- **WHEN** the `Data` of `Абдулов (директор)` (УНФ demo) is decoded
- **THEN** the name is `Абдулов (директор)`, the full name
  `Абдулов Юрий Владимирович`, and the roles are the three identifiers
  named `АдминистраторСистемы`, `ПолныеПрава` and
  `ИнтерактивноеОткрытиеВнешнихОтчетовИОбработок` by the catalog

#### Scenario: A user without roles
- **WHEN** the `Data` of a user whose role block is `{0}` is decoded
- **THEN** the role list is empty

#### Scenario: A malformed blob
- **WHEN** the blob is shorter than its key length
- **THEN** decoding fails with a `MetadataError`

#### Scenario: E-mail of a user
- **WHEN** the row of a user carries `Email`
- **THEN** `InfoBaseUser::email` is that text, and `\users` and `\user`
  print it

### Requirement: Expand a restriction text
The library SHALL expand a restriction text of a role into a condition:
a template call `#Имя(аргументы)` SHALL be replaced by the body of the
role's template of that name with `#Параметр(N)` and the named
parameters of its signature replaced by the arguments, recursively up to
a bounded depth; `#ИмяТекущейТаблицы` and `#ТекущаяТаблица` SHALL stand
for the name of the restricted table and `#ИмяТекущегоПраваДоступа` for
the Russian name of
the right; `#Если <выражение> #Тогда`, `#ИначеЕсли`, `#Иначе` and
`#КонецЕсли` SHALL keep the branch whose expression holds, where an
expression combines `&Параметр` session values, string literals, the
three names above, `Истина`/`Ложь`, `+`, `=`, `<>`, `Не`, `И`, `Или`,
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

### Requirement: Rights not listed follow the role default
A rights resource records only what differs from the role's default: a
right the resource does not list for an object, and every right of an
object it does not list at all, SHALL count as granted when
`setForNewObjects` holds and as refused otherwise. `RoleRights::grants`
SHALL answer so, and `read_access` SHALL use it, so that `ПолныеПрава`
— which lists nothing but its refusals — grants `Чтение` of every table.

#### Scenario: Full rights role
- **WHEN** `ПолныеПрава` (УНФ) lists only refused interactive deletions
  for `Справочник.Организации`
- **THEN** it grants `Чтение` of the catalog and refuses
  `ИнтерактивноеУдаление`

#### Scenario: Read-only role
- **WHEN** a role without `setForNewObjects` does not list an object
- **THEN** it grants no right on it

### Requirement: Parse a restriction text into its nodes
The library SHALL parse a restriction text or a template body into a
sequence of nodes, each carrying its byte offset in the text: literal
text; a condition `#Если <выражение> #Тогда … [#ИначеЕсли …] [#Иначе …]
#КонецЕсли` with the expression and the body of every branch and the
body of the alternative; a call `#Имя(аргументы)` of a template the role
carries, with the arguments unquoted as the expansion reads them; a
numbered parameter `#Параметр(N)`; and any other `#Имя` as a name. A call
SHALL be read by the same scanner the expansion uses, so a text that
expands one way never parses another. Comments SHALL be dropped as the
expansion drops them.

A text whose directives do not balance SHALL be an error naming what is
missing and the byte offset at which it is missing.

#### Scenario: Branches and calls
- **WHEN** the restriction of `ЧтениеЭлектронныхДокументов` is parsed
  with the templates of the role
- **THEN** the nodes are one condition whose first branch reads
  `&ОграничениеДоступаНаУровнеЗаписейУниверсально` and whose bodies call
  `ДляОбъекта`

#### Scenario: Unbalanced directives
- **WHEN** a text holds `#Если &П #Тогда ИСТИНА` without `#КонецЕсли`
- **THEN** parsing fails naming `#КонецЕсли` and the offset past the text

#### Scenario: A name is not a call
- **WHEN** a body holds `#ПолеОбъекта` and `#Параметр(2)`
- **THEN** the first is a name node and the second a numbered parameter
