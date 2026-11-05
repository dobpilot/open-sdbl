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

#### Scenario: Independently measured configuration rights
- **WHEN** native single-right role resources are decoded
- **THEN** d8682bbb-7800-4aa0-8590-d3cb11fe2a29 is SaveUserData
- **AND** 399d7390-8d83-4a57-b4d7-c902c15b701f is ConfigurationExtensionsAdministration
- **AND** 7b8359dd-7d4e-4bcd-a61c-b4b26eae19c6 is InteractiveOpenExtDataProcessors
- **AND** eb29e198-c338-4a20-a253-be6fc3dd44d9 is InteractiveOpenExtReports
- **AND** the named grants match the retained native positive and cross-negative checks

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

### Requirement: Answer a restricted compilation with decisions
The database layer SHALL answer the restriction request of a restricted
compilation with one decision per target, derived from the access a user's
roles grant for `Чтение`: a role granting the right without a restriction
SHALL yield the unrestricted decision, restrictions SHALL yield the
expanded condition, and a right no role grants SHALL yield the denied
decision. The absence of a current user, a role whose rights the base does
not carry, or any restriction-expansion error SHALL fail the whole answer.
A partially expanded set SHALL NOT be offered as sufficient for execution,
and missing data SHALL NOT be read as permission.

#### Scenario: A role grants the right outright
- **WHEN** one of the user's roles grants `Чтение` of an object without a
  restriction
- **THEN** the answer carries the unrestricted decision for that target

#### Scenario: No role grants the right
- **WHEN** no role of the user grants `Чтение` of a requested object
- **THEN** the answer carries the denied decision for that target

#### Scenario: An expansion that fails
- **WHEN** the restriction text of one target cannot be expanded
- **THEN** the whole answer fails, naming the target and the reason, and
  no decision set is returned

#### Scenario: A denial admits no row on the server
- **WHEN** a restricted query whose only target is denied runs against a
  live base
- **THEN** the server returns no row of that table, while the same query
  with the unrestricted decision returns rows

#### Scenario: No current user
- **WHEN** a restricted compilation is requested with no current user
- **THEN** the answer fails rather than returning an empty decision set

### Requirement: Answer whether a user can authenticate
`InfoBaseUser` SHALL answer whether the user has any way to log in: it
SHALL be true when standard 1C authentication is on, or when the
operating-system login is not blank once trimmed, and false otherwise.
The answer SHALL NOT depend on whether the user is shown in the login
list, on the administrative flag, on the roles the user holds, or on the
name. The administrative flag is a right, not a way in.

The rule SHALL live on `InfoBaseUser`; the database package and the
console SHALL ask it rather than restate it.

#### Scenario: Standard authentication
- **WHEN** a user has standard authentication on and no operating-system
  login
- **THEN** the user can authenticate

#### Scenario: Operating-system login
- **WHEN** a user has standard authentication off and an
  operating-system login
- **THEN** the user can authenticate

#### Scenario: Both ways
- **WHEN** a user has both
- **THEN** the user can authenticate

#### Scenario: Neither way
- **WHEN** a user has standard authentication off and a blank
  operating-system login
- **THEN** the user cannot authenticate

#### Scenario: Flags that do not decide
- **WHEN** the same user is asked with the show-in-list and
  administrative flags set and cleared
- **THEN** the answer is the same in every combination

### Requirement: Preserve missing restriction session parameter identity
When restriction expansion reads a session parameter without a value, the database layer SHALL return a structured error containing that parameter's name. Other restriction errors SHALL retain their existing diagnostic category and readable context.

#### Scenario: RLS parameter is absent
- **WHEN** a selected user's `Read` restriction references an unset session parameter
- **THEN** deriving its access fails with a structured missing-session-parameter error carrying the original name

### Requirement: Check every field read for the selected user
The database layer SHALL check the complete `FieldUsageRequest` against the selected user's loaded role rights and the metadata snapshot. It SHALL return success only when every read is proven permitted; an absent current user, unread rights, unresolved field, explicit denial without another granting role, or unsupported member path SHALL fail the whole check. It SHALL NOT return a partial authorization result or treat table-level `Read` as sufficient for a custom attribute with an explicit member `Read` refusal. Measured native catalog mappings MAY authorize supported programmatic reads independently of UI rights.

#### Scenario: A denied attribute is projected
- **WHEN** a role grants object `Read` but explicitly refuses `Read` of a projected custom attribute, and no other role grants that attribute
- **THEN** the complete field check fails before SQL execution

#### Scenario: A second role grants the attribute
- **WHEN** one role refuses an attribute and another role explicitly grants `Read` on both the object and its exact attribute member GUID
- **THEN** that field passes the check

#### Scenario: A field is used only for filtering
- **WHEN** a denied attribute appears in `WHERE` but not the projection
- **THEN** the complete field check fails

#### Scenario: Rights are not loaded
- **WHEN** any role of the selected user has unread or unavailable rights
- **THEN** the complete field check fails rather than treating the missing role as a denial or grant

#### Scenario: An unsupported member path
- **WHEN** a field's rights cannot be mapped unambiguously from the decoded resource and snapshot
- **THEN** the complete field check fails naming the field and reason

#### Scenario: A default flag without an explicit member grant
- **WHEN** an object grants `Read` and its role sets `setForAttributesByDefault`, but an unverified custom attribute has no exact member `Read` grant
- **THEN** the field check fails rather than inferring platform field semantics from the flag

#### Scenario: A standard field without a verified member mapping
- **WHEN** a prepared query reads a standard field without a Config GUID or a measured native mapping, even if the object has `Read`
- **THEN** the field check fails closed and names the mapping limitation

### Requirement: Authorize the value of a verified constant by object Read
When a field read resolves to the value of a constant and its metadata field identifier equals that constant's identifier, the database layer SHALL authorize the value through the selected user's role object `Read` grants. This mapping SHALL NOT depend on `setForAttributesByDefault`. Unresolved fields, mismatched identifiers, unread rights and unsupported member paths SHALL remain fail closed. A role explicitly denying object `Read` SHALL NOT grant the value; a different role granting object `Read` MAY grant it.

#### Scenario: The constant value is read without a member entry
- **WHEN** a role grants `Read` of a verified constant and no separate member entry exists
- **THEN** reading its value passes regardless of the default attribute flag

#### Scenario: No role grants the constant
- **WHEN** all selected roles deny the constant's object `Read`
- **THEN** reading its value fails before SQL execution

#### Scenario: Another role grants the constant
- **WHEN** one role denies and another grants object `Read` of the constant
- **THEN** its value passes the field check

#### Scenario: The identifier belongs to another object
- **WHEN** a requested field is not the resolved value of that constant
- **THEN** the constant exception does not authorize it

### Requirement: Preserve unsupported custom member authorization
Until a native member rule is implemented with its measured mapping, custom attributes and standard fields SHALL retain their existing authorization boundary. Native `View` permissions SHALL NOT be renamed into member `Read` permissions.

#### Scenario: A custom attribute has no exact member Read grant
- **WHEN** an unmeasured custom attribute has no supported exact member `Read` grant
- **THEN** the field check retains its existing fail-closed result

### Requirement: Authorize measured native member mappings
The SDK SHALL use verified native standard-field and default-attribute permission mappings when available. Explicit denials, missing rights, unresolved fields, unsupported paths and unknown defaults SHALL remain fail closed. Object Read alone SHALL NOT be accepted as proof for unmeasured standard fields. RLS and separation checks SHALL remain applicable.

#### Scenario: Verified standard field
- **WHEN** a field has a measured native member mapping and a selected role grants its required permissions
- **THEN** the field passes without bypassing object or row authorization

#### Scenario: Unsupported mapping
- **WHEN** the field or default rule is not measured
- **THEN** authorization fails before SQL execution

### Requirement: Map verified catalog programmatic reads
For a recognized catalog Config owner with complete own attribute membership and standard settings, the SDK SHALL map existing Ref, Code, Description and DeletionMark fields, and own custom GUIDs with recognized declarations, to the role's object Read authorization when no explicit supported member Read overrides it. This rule SHALL NOT depend on attribute-default flags or UI View/Edit rights. Unverified snapshots, other standard fields and ambiguous member paths SHALL remain fail closed.

#### Scenario: Native catalog has a denied UI View
- **WHEN** the verified own attribute has an explicit UI View refusal but the role grants object Read and has no member Read refusal
- **THEN** its programmatic field read passes without renaming View to Read

#### Scenario: Native catalog has defaults off
- **WHEN** a role grants object Read of a verified catalog and attribute defaults are false
- **THEN** the supported standard and own fields pass the programmatic check

#### Scenario: Object Read is refused
- **WHEN** no selected role grants object Read
- **THEN** all field reads fail before SQL

#### Scenario: No verified Config owner
- **WHEN** a compact or unknown snapshot cannot prove the native catalog membership and settings
- **THEN** the existing unsupported mapping refusal remains

### Requirement: Map verified accumulation register programmatic reads
For a recognized native accumulation register owner with verified Balance register kind and complete own dimension/resource collections, the SDK SHALL authorize reads of those own GUID fields with nonempty recognized declarations through the selected roles' object Read grants. This rule SHALL NOT depend on UI View or attribute-default flags. Missing users, unread roles, unresolved membership, ambiguous member paths and unsupported standard fields SHALL remain fail closed. Supported explicit member Read refusals SHALL retain their conservative refusal. Field checks SHALL NOT bypass existing RLS decisions or query compilation.

#### Scenario: Owner Read grants dimension and resource
- **WHEN** a selected role grants owner Read for verified dimension/resource fields with View false and either attribute-default flag
- **THEN** the complete field check succeeds

#### Scenario: Owner Read is absent
- **WHEN** no selected role grants owner Read
- **THEN** reading either field fails before SQL, including an ALLOWED query

#### Scenario: Native membership is unknown
- **WHEN** a field is not proven by complete native dimension/resource collections and a recognized declaration
- **THEN** the native mapping does not grant it

#### Scenario: RLS remains applicable
- **WHEN** verified fields pass their field checks and selected roles restrict owner Read
- **THEN** existing access decisions preserve the restrictions and combine multiple granting roles with OR

### Requirement: Verify only confirmed supported stored-password values
The dependency-free, I/O-free core SHALL verify public `StoredPasswordValue`
using the supplied Python v0.2 algorithm: legacy SHA-1 and IDs 1–3.
`UserData::verify_password` SHALL use only an already-confirmed mapping
from a user's data to that public property; otherwise it SHALL return
`INDETERMINATE` / `SOURCE_PROFILE_UNCONFIRMED`. This change SHALL NOT
reverse engineer records, measure probe bases, discover password-field
positions, or infer provenance from hash-shaped text.

#### Scenario: Public property is supplied
- **WHEN** a caller supplies a public `StoredPasswordValue` with the
  `public-stored-value-8.3.27` source profile
- **THEN** verification checks the value under its supported algorithm
  without reading or modifying a database

#### Scenario: Unconfirmed blob-derived value
- **WHEN** the source profile is `raw-blob`, any other unrecognized value,
  or the `UserData` adapter has no confirmed source mapping
- **THEN** verification returns `INDETERMINATE` /
  `SOURCE_PROFILE_UNCONFIRMED` even if the text resembles valid `H1,H2`

#### Scenario: Synthetic evidence stays synthetic
- **WHEN** a fixture uses either accepted synthetic source profile
- **THEN** it can verify the algorithm but does not claim a known
  `v8users.Data` password-field position or successful live authentication

### Requirement: Use explicit verification profiles and defaults
Accepted sources SHALL be `public-stored-value-8.3.27`,
`synthetic-public-stored-value-8.3.27`, `public-stored-value-legacy`, and
`synthetic-public-stored-value-legacy`; others SHALL be unconfirmed.
Defaults SHALL be the first source, password state `unknown`, secondary
mode `NONE`, and unset `record_id`. Sources SHALL assert provenance, not
select algorithms. The `UserData` adapter SHALL NOT default unknown blob
mappings to confirmed sources.

#### Scenario: Default public-property verification
- **WHEN** a caller supplies the public property and candidate without
  overriding verification options
- **THEN** the source is `public-stored-value-8.3.27`, password state is
  `unknown`, secondary mode is `NONE`, and `record_id` is unset

#### Scenario: Legacy source names remain accepted
- **WHEN** either accepted legacy source name is used with a supported
  two-field or four-field value
- **THEN** verification selects the algorithm from the record format and
  ID, without limiting that source name to SHA-1

### Requirement: Preserve input and validate bounded verification options
Verification SHALL accept strings, source profile, password state,
secondary mode, optional non-secret string `record_id`, and nonnegative
integer byte limits. It SHALL validate types/options, character counts,
strict UTF-8 encoding, then byte counts before checking the source.
Defaults SHALL be 1,048,576 candidate bytes and 65,536 stored-value bytes.
Passwords SHALL NOT be trimmed, normalized, case-folded, or truncated.
Rust types MAY make invalid inputs unrepresentable.

#### Scenario: UTF-8 byte limit
- **WHEN** the candidate is `é` and its byte limit is 1
- **THEN** verification returns `LIMIT_EXCEEDED` / `INPUT_TOO_LARGE`
  although the candidate has only one character

#### Scenario: Embedded NUL is part of the password
- **WHEN** a candidate is `a\0b` and H1 is the SHA-1 digest of bytes
  `61 00 62` encoded in canonical Base64
- **THEN** verification hashes all three bytes without truncation

#### Scenario: Whitespace and Unicode are preserved
- **WHEN** candidates differ by a trailing newline, leading whitespace,
  case, or Unicode normalization form
- **THEN** verification hashes each exact UTF-8 byte sequence without
  transforming it

#### Scenario: Invalid input at an untyped boundary
- **WHEN** an adapter accepts a non-string candidate or a negative byte
  limit from an untyped caller
- **THEN** it returns `INVALID_INPUT` / `EXPECTED_STRINGS` or
  `INVALID_INPUT` / `INVALID_LIMIT`, respectively

### Requirement: Preserve input-validation failure reasons
Type/option checks SHALL precede size checks, with `INVALID_INPUT` reasons
`EXPECTED_STRINGS`, `INVALID_METADATA`, `INVALID_PASSWORD_STATE`,
`INVALID_SECONDARY_MODE`, then `INVALID_LIMIT`. Invalid metadata SHALL
clear `record_id`. Invalid Unicode SHALL yield `INVALID_INPUT` /
`INVALID_UNICODE` after character limits and before byte limits.
Exceeded limits SHALL yield `LIMIT_EXCEEDED` / `INPUT_TOO_LARGE` before
source checks or hashing. Limits SHALL be nonnegative integers.

#### Scenario: Invalid metadata clears the identifier
- **WHEN** an untyped adapter receives non-string `record_id`, source
  profile, or secondary mode
- **THEN** it returns `INVALID_INPUT` / `INVALID_METADATA` with no
  `record_id`

#### Scenario: Invalid state and mode are distinct
- **WHEN** an untyped caller supplies password state `yes`, secondary
  mode `UNICODE_UPPER`, or a negative limit in separate calls
- **THEN** the reasons are respectively `INVALID_PASSWORD_STATE`,
  `INVALID_SECONDARY_MODE`, and `INVALID_LIMIT`

#### Scenario: Invalid Unicode is rejected where representable
- **WHEN** an untyped boundary receives an isolated Unicode surrogate
  within the character-count limit
- **THEN** it returns `INVALID_INPUT` / `INVALID_UNICODE`

### Requirement: Distinguish unknown and explicitly absent passwords
After input validation, verification SHALL check source, explicit password
state, then empty stored value, in that order. A confirmed source with
state `false` SHALL yield `NO_PASSWORD_CONFIGURED` /
`EXPLICIT_PASSWORD_NOT_SET`. Empty stored content with state `true` or
`unknown` SHALL yield `INDETERMINATE` / `EMPTY_STORED_VALUE`. An empty
property, unknown state, or authentication flag SHALL NOT imply a stored
SHA-1 digest of the empty password.

#### Scenario: Explicitly unset password
- **WHEN** the source is confirmed and password state is `false`
- **THEN** even an empty candidate and a valid H1 yield
  `NO_PASSWORD_CONFIGURED` / `EXPLICIT_PASSWORD_NOT_SET`

#### Scenario: Empty property is unknown
- **WHEN** the source is confirmed, password state is `unknown`, and the
  stored value is empty
- **THEN** the result is `INDETERMINATE` / `EMPTY_STORED_VALUE`

#### Scenario: Validation precedes password state
- **WHEN** an oversized candidate is supplied with password state `false`
- **THEN** the result is `LIMIT_EXCEEDED` / `INPUT_TOO_LARGE`

#### Scenario: Source check precedes password state
- **WHEN** bounded valid inputs have an unconfirmed source and password
  state `false`
- **THEN** the result is `INDETERMINATE` / `SOURCE_PROFILE_UNCONFIRMED`

### Requirement: Parse stored-password fields without algorithm fallback
Verification SHALL split on literal commas, preserving empty/trailing
fields without trimming. Counts other than two or four SHALL yield
`INVALID_RECORD` / `INVALID_FIELD_COUNT`. Two fields SHALL select unsalted
SHA-1. Four fields SHALL be `H1,H2,algorithmId,salt`; the ID SHALL match
ASCII `[0-9]+`, else `INVALID_RECORD` / `INVALID_ALGORITHM_ID`. Supported
IDs SHALL select their algorithm without fallback to legacy or another ID.

#### Scenario: Extended profiles select their algorithm
- **WHEN** a four-field record has algorithm ID 1, 2, or 3
- **THEN** verification selects SHA-256, SHA-512, or PBKDF2-HMAC-SHA256,
  respectively, with the fourth field as literal salt

#### Scenario: Invalid algorithm identifier
- **WHEN** a four-field value has third field `+1`, an empty string, or
  non-ASCII digits
- **THEN** verification returns `INVALID_RECORD` / `INVALID_ALGORITHM_ID`

#### Scenario: Trailing separator is retained
- **WHEN** a valid two-field value is followed by one comma
- **THEN** verification sees three fields and returns `INVALID_RECORD` /
  `INVALID_FIELD_COUNT`

### Requirement: Normalize decimal IDs and reject unsupported profiles
Four-field IDs SHALL have leading zeros removed, with an all-zero ID
becoming `0`, without bounded-integer conversion of the original text.
ID 0 SHALL yield `UNSUPPORTED_PROFILE` / `NONCANONICAL_LEGACY_RECORD`;
IDs outside 1–3 SHALL yield `UNSUPPORTED_PROFILE` / `UNKNOWN_ALGORITHM_ID`.
These returns SHALL precede digest decoding and leave profile unset.
Unknown algorithms SHALL NOT fall back to the record's first two fields.

#### Scenario: Leading zeros do not change a supported ID
- **WHEN** algorithm IDs `0001`, `0002`, `0003`, or an ID containing
  thousands of leading zeros before `3` are supplied within input limits
- **THEN** they select IDs 1, 2, and 3 without integer conversion overflow

#### Scenario: Four-field legacy record is noncanonical
- **WHEN** a four-field value has algorithm ID `0` or `000`
- **THEN** the result is `UNSUPPORTED_PROFILE` /
  `NONCANONICAL_LEGACY_RECORD`, even if H1 would match SHA-1

#### Scenario: Unknown algorithm cannot reuse a valid H1
- **WHEN** algorithm ID is `4` or an arbitrarily long nonzero decimal ID
  within the stored-value limit
- **THEN** the result is `UNSUPPORTED_PROFILE` / `UNKNOWN_ALGORITHM_ID`,
  without integer conversion overflow or digest comparison

### Requirement: Preserve literal salt and unsalted legacy semantics
For four fields, salt SHALL be the exact UTF-8 bytes of the fourth field,
including case and whitespace; it SHALL NOT be Base64/hex decoded,
uppercased, or normalized. Algorithm input SHALL be candidate UTF-8 bytes
followed by salt bytes, without a delimiter. Empty salt SHALL be accepted.
The two-field `H1,H2` legacy format SHALL have no salt and SHALL hash only
candidate bytes.

#### Scenario: Salt abc123 is literal text
- **WHEN** candidate `PaSs123!` has salt field `abc123`
- **THEN** the primary algorithm input is the UTF-8 encoding of
  `PaSs123!abc123`, without Base64 decoding `abc123`

#### Scenario: Base64-looking salt and empty salt
- **WHEN** the salt is `YWJjMTIz` or empty
- **THEN** the salt bytes are respectively literal UTF-8 `YWJjMTIz` or
  zero bytes, rather than decoded `abc123` or missing-record errors

#### Scenario: Legacy record has no salt
- **WHEN** candidate `PaSs123!` is checked against a two-field value
- **THEN** the SHA-1 input is exactly `PaSs123!`, with no salt appended

### Requirement: Compute the selected algorithm with fixed parameters
Legacy SHA-1 SHALL use profile `LEGACY_SHA1` and 20 digest bytes; ID 1
SHALL use `SHA256` and 32 bytes; ID 2 SHALL use `SHA512` and 64 bytes.
ID 3 SHALL use profile `PBKDF2_HMAC_SHA256`, HMAC-SHA256, 16,000 iterations,
and 64 derived bytes. PBKDF2's password SHALL be the concatenated candidate
and literal record salt; its separate salt argument SHALL be empty.

#### Scenario: PBKDF2 does not use the record salt as a separate argument
- **WHEN** candidate `PaSs123!` has ID 3 and salt `abc123`
- **THEN** H1 is compared with
  `PBKDF2-HMAC-SHA256(UTF8("PaSs123!abc123"), salt=b"", iterations=16000, dkLen=64)`
  rather than PBKDF2 with password `PaSs123!` and separate salt `abc123`

### Requirement: Bound combined PBKDF2 input before digest decoding
After selecting ID 3 and setting its profile, verification SHALL enforce a
fixed 1,048,576-byte limit on candidate bytes plus literal salt bytes.
Exceeding it SHALL yield `LIMIT_EXCEEDED` / `PBKDF2_INPUT_TOO_LARGE` with
profile `PBKDF2_HMAC_SHA256` before decoding either digest. Raising caller
input limits SHALL NOT raise this limit. Other algorithms SHALL use the
ordinary candidate/stored-value limits without this combined-input check.

#### Scenario: Salt makes an otherwise valid candidate too large
- **WHEN** ID 3 has a 1,048,576-byte candidate and one-byte salt, with
  ordinary input limits satisfied
- **THEN** verification returns `PBKDF2_INPUT_TOO_LARGE` before digest
  decoding, including when the digest fields themselves are malformed

#### Scenario: Exact boundary is allowed
- **WHEN** candidate plus salt is exactly 1,048,576 bytes for ID 3
- **THEN** verification proceeds to digest validation

### Requirement: Validate both digests as canonical Base64
After algorithm selection and the PBKDF2 input check, verification SHALL
decode H1 then H2 as strict standard Base64 and require exact re-encoding
including pad bits. Invalid encoding SHALL yield `INVALID_RECORD` /
`INVALID_BASE64`; a canonical digest of the wrong algorithm-specific
length SHALL yield `INVALID_RECORD` / `INVALID_DIGEST_LENGTH`. Both fields
SHALL be validated before hashing, even when secondary checking is off.

#### Scenario: Malformed H2 cannot be bypassed
- **WHEN** H1 matches the candidate but H2 is `!`, with secondary mode
  `NONE`
- **THEN** verification returns `INVALID_RECORD` / `INVALID_BASE64`
  without reporting a primary match

#### Scenario: Noncanonical pad bits
- **WHEN** the first field is `2jmj7l5rSw0yVb/vlWAYkK/YBWl=`
  and the second is `2jmj7l5rSw0yVb/vlWAYkK/YBwk=`
- **THEN** verification rejects the first field as `INVALID_BASE64`
  even if a permissive decoder would produce the empty-password digest

#### Scenario: Canonical digest with wrong length
- **WHEN** either field decodes to a length other than 20 for legacy,
  32 for ID 1, or 64 for ID 2 or 3
- **THEN** verification returns `INVALID_RECORD` /
  `INVALID_DIGEST_LENGTH`

#### Scenario: Base64 must reproduce the original field
- **WHEN** a field contains whitespace, URL-safe substitutions, invalid
  characters, missing or extra padding, or noncanonical pad bits
- **THEN** verification returns `INVALID_RECORD` / `INVALID_BASE64`

### Requirement: Authorize only by the selected primary digest
Verification SHALL compare the selected algorithm's digest to H1 using a
constant-time comparison for its digest length. H1 alone SHALL determine `MATCH`
/ `PRIMARY_DIGEST_MATCH` or `NO_MATCH` / `PRIMARY_DIGEST_MISMATCH`.
H2 SHALL NOT authorize or override H1. An unavailable hash SHALL yield
`INDETERMINATE` / `HASH_UNAVAILABLE`, the selected profile, and no match
assertions. Whether the user may log in SHALL remain a separate
`InfoBaseUser` and gateway policy decision.

#### Scenario: Supplied synthetic demo
- **WHEN** the confirmed synthetic stored value is
  `IaYx2h3Z5OS8l6IvFucwUQxq2E0=,pfQAOj19NcDRSmGz4CwWTv6dRSk=`
  and the candidate is `PaSs123!`
- **THEN** the result is `MATCH` / `PRIMARY_DIGEST_MATCH`

#### Scenario: Uppercase variant cannot log in
- **WHEN** the same synthetic value is checked with `PASS123!` and
  `ASCII_UPPER_DIAGNOSTIC`
- **THEN** the result is `NO_MATCH`, even though the H2 comparison matches

#### Scenario: Explicit SHA-1 of an empty candidate
- **WHEN** the confirmed value is
  `2jmj7l5rSw0yVb/vlWAYkK/YBwk=,2jmj7l5rSw0yVb/vlWAYkK/YBwk=`,
  password state is `true` or `unknown`, and the candidate is empty
- **THEN** the result is `MATCH`, while a nonempty candidate yields
  `NO_MATCH`

### Requirement: Keep secondary checking diagnostic and ASCII-only
`NONE` SHALL leave `uppercase_match` unset and consistency `not_checked`.
`ASCII_UPPER_DIAGNOSTIC` SHALL compare the selected digest of uppercase
ASCII candidate bytes plus unchanged salt to H2 in constant time (map
only `a`–`z` to `A`–`Z`). Consistency SHALL be `not_established` unless H1
matches and H2 was compared, then `consistent` or `inconsistent`.
H2 SHALL NOT alter H1's result. Non-ASCII H2 checks SHALL be skipped.

#### Scenario: Both synthetic digests agree
- **WHEN** the supplied demo is checked with `PaSs123!` and
  `ASCII_UPPER_DIAGNOSTIC`
- **THEN** `primary_match` and `uppercase_match` are true and
  `secondary_consistency` is `consistent`

#### Scenario: Inconsistent H2 does not reject H1
- **WHEN** the demo's H2 is replaced by the canonical empty-password
  digest and the original candidate is checked with
  `ASCII_UPPER_DIAGNOSTIC`
- **THEN** the result is `MATCH`, `uppercase_match` is false, and
  `secondary_consistency` is `inconsistent`

#### Scenario: Non-ASCII secondary comparison is skipped
- **WHEN** a non-ASCII candidate is checked with `ASCII_UPPER_DIAGNOSTIC`
- **THEN** `uppercase_match` is unset, consistency is `not_established`,
  and H1 is still checked against the exact UTF-8 candidate followed by
  unchanged salt when present

#### Scenario: H2 match without H1 match proves no consistency
- **WHEN** H1 mismatches but the ASCII diagnostic comparison matches H2
- **THEN** the result is `NO_MATCH`, `uppercase_match` is true, and
  `secondary_consistency` remains `not_established`

#### Scenario: Uppercase diagnostics preserve salt in extended records
- **WHEN** candidate `PaSs123!`, salt `abc123`, and
  `ASCII_UPPER_DIAGNOSTIC` are used with ID 1, 2, or 3
- **THEN** the H2 algorithm input is UTF-8 `PASS123!abc123`, preserving
  the salt's lowercase letters; H1 still decides the result

#### Scenario: Non-ASCII salt does not disable ASCII diagnostics
- **WHEN** the candidate is ASCII `PaSs123!` but salt is `ёabc123`
- **THEN** the H2 diagnostic still runs on UTF-8 `PASS123!ёabc123`,
  preserving the salt exactly

### Requirement: Return typed verification outcomes without credentials
Results SHALL carry status, reason, profile, optional non-secret
`record_id`, `primary_match`, `uppercase_match`, `secondary_consistency`,
and `verified` (true only for `MATCH`, false only for `NO_MATCH`, otherwise
unset). All eight statuses and reason codes in the design SHALL remain
distinct. Results, public getters, `Debug`, errors, and logs SHALL NOT
expose passwords or stored hashes. Verification SHALL perform no I/O or
modify user/password records.

#### Scenario: Failure is distinct from a password mismatch
- **WHEN** H2 is malformed, the source is unconfirmed, or a profile is
  unsupported
- **THEN** `verified` and both match fields are unset, rather than false,
  and the typed status and reason identify the respective failure

#### Scenario: Early return has no match assertions
- **WHEN** verification returns before primary comparison
- **THEN** match fields are unset and consistency is `not_checked`;
  profile is unset until a supported algorithm is selected, then identifies
  that algorithm even for digest errors or the PBKDF2 input limit

#### Scenario: Credential material stays private
- **WHEN** the supplied demo is verified and its result, `UserData`, and
  any error are formatted for diagnostics
- **THEN** none exposes `PaSs123!`, the stored value, or either stored hash

#### Scenario: Caller identifier is retained safely
- **WHEN** a caller supplies non-secret `record_id` `synthetic-user-1`
- **THEN** the result retains that identifier without deriving an
  identifier from the password or hashes
