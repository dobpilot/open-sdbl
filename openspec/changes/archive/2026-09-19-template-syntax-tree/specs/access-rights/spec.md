## ADDED Requirements

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
