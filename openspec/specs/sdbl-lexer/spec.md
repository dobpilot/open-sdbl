# sdbl-lexer Specification

## Purpose

Define the observable lexical-analysis contract shared by the Rust library and
the `open-sdbl` command-line interface.

## Requirements

### Requirement: Tokenize the supported SDBL lexical subset

The library SHALL tokenize identifiers, parameters, strings, numbers,
operators, punctuation, and comments while discarding whitespace.

#### Scenario: Representative query

- **WHEN** a caller tokenizes a query containing all supported token classes
- **THEN** the returned tokens preserve their class, original text, byte span,
  and one-based line and column

### Requirement: Classify bilingual keywords

The library SHALL classify supported Russian and English SDBL keywords without
regard to Unicode letter case.

#### Scenario: Keyword aliases

- **WHEN** a query contains `ВЫБРАТЬ`, `выбрать`, or `SELECT`
- **THEN** each spelling is returned as the same `Select` keyword

### Requirement: Report malformed input

The library SHALL stop at malformed strings, parameters, malformed binary
literals, or unsupported characters and return a diagnostic with its source
position and a machine-readable category.

#### Scenario: Unterminated string

- **WHEN** the input ends inside a string literal
- **THEN** tokenization returns an unterminated-string diagnostic at the
  opening quote

#### Scenario: Unexpected character

- **WHEN** the input contains a character outside the supported lexical
  subset
- **THEN** tokenization returns an unexpected-character diagnostic carrying
  that character and its one-based line and column

### Requirement: Expose lexical analysis through the CLI

The executable SHALL provide `open-sdbl lex [FILE|-]`, reading the named file
or standard input and printing one tab-separated token per line.

#### Scenario: Lex a file

- **WHEN** a readable file is passed to `open-sdbl lex`
- **THEN** its tokens are written to standard output and the process exits
  successfully

#### Scenario: Invalid source

- **WHEN** lexical analysis fails
- **THEN** a positional diagnostic is written to standard error and the
  process exits unsuccessfully

### Requirement: Recognize COUNT bilingually
The lexer SHALL classify `COUNT` and `КОЛИЧЕСТВО` case-insensitively as one
aggregate keyword while preserving the original lexeme and span.

#### Scenario: English and Russian aggregate names
- **WHEN** input contains `count` or `Количество`
- **THEN** both tokens have the COUNT keyword kind and retain their spelling

### Requirement: Recognize basic aggregates bilingually
The lexer SHALL classify `SUM`/`СУММА`, `MIN`/`МИНИМУМ`, and
`MAX`/`МАКСИМУМ` case-insensitively as their aggregate keyword kinds while
preserving original spelling and span.

#### Scenario: Russian and English aggregate names
- **WHEN** input contains each Russian and English aggregate spelling
- **THEN** every token has its corresponding aggregate keyword kind

### Requirement: Recognize SliceLast keywords
The lexer SHALL classify `СрезПоследних` and `SliceLast`, case-insensitively,
as the same SliceLast keyword while preserving the exact source lexeme and
span.

#### Scenario: Bilingual SliceLast spelling
- **WHEN** Russian and English SliceLast spellings are tokenized
- **THEN** both tokens have the SliceLast keyword kind and retain their original
  text

### Requirement: Recognize SliceFirst keywords
The lexer SHALL classify `СрезПервых` and `SliceFirst`, case-insensitively,
as the same SliceFirst keyword while preserving exact source spelling and span.

#### Scenario: Bilingual SliceFirst spelling
- **WHEN** Russian and English SliceFirst spellings are tokenized
- **THEN** both tokens have the SliceFirst keyword kind and retain their
  original text

### Requirement: Recognize accumulation virtual-table keywords
The lexer SHALL classify `Остатки`/`Balance` and `Обороты`/`Turnovers`
case-insensitively as their respective keyword kinds while preserving spelling
and span.

#### Scenario: Bilingual accumulation virtual tables
- **WHEN** Russian and English Balance and Turnovers spellings are tokenized
- **THEN** every token has its corresponding keyword kind and original text

### Requirement: Iterate tokens through the standard iterator protocol
The lexer SHALL be usable as a standard iterator yielding successful tokens
or a diagnostic, and SHALL yield nothing after the first diagnostic.

#### Scenario: Iterating a source with an error
- **WHEN** a caller iterates a source whose third token is malformed
- **THEN** the iterator yields two tokens, then one diagnostic, then no
  further items

### Requirement: Recognize the full bilingual keyword table
The lexer SHALL classify every supported keyword's Russian and English
spellings case-insensitively, and the mapping SHALL be fixed by tests
covering every keyword variant in both languages.

#### Scenario: Grouping and set-operation keywords
- **WHEN** input contains `СГРУППИРОВАТЬ`, `ИМЕЮЩИЕ`, `ОБЪЕДИНИТЬ`,
  `ПОМЕСТИТЬ`, and their English spellings
- **THEN** each token receives its corresponding keyword kind while
  preserving the original lexeme

### Requirement: Recognize the reference UUID keyword bilingually
The lexer SHALL classify `УНИКАЛЬНЫЙИДЕНТИФИКАТОР` and `UUID`
case-insensitively as one keyword kind whose stable display name is `UUID`,
and the exhaustive keyword table test SHALL include both spellings.

#### Scenario: Russian and English spellings
- **WHEN** input contains `УникальныйИдентификатор(Ссылка)` or `uuid(Ref)`
- **THEN** the function name is a keyword token that preserves the original
  lexeme

### Requirement: Recognize hexadecimal binary literals
The lexer SHALL return `0x`/`0X` followed by a non-empty even number of ASCII
hexadecimal digits as one binary-literal token while preserving its original
spelling and source span. Malformed binary literals SHALL produce a positional
diagnostic instead of being split into number and identifier tokens.

#### Scenario: Rowversion literal
- **WHEN** a query contains `0x00000000000007D6`
- **THEN** the lexer returns one binary-literal token containing the complete
  spelling

#### Scenario: Malformed binary literal
- **WHEN** a `0x` literal is empty, has an odd digit count, or contains a
  non-hexadecimal identifier character
- **THEN** tokenization returns an invalid-binary-literal diagnostic at the
  `0x` prefix

### Requirement: Recognize the cast keyword bilingually
The lexer SHALL classify `ВЫРАЗИТЬ` and `CAST` case-insensitively as one
keyword kind whose stable display name is `CAST`, and the exhaustive keyword
table test SHALL include both spellings.

#### Scenario: Russian and English spellings
- **WHEN** input contains `Выразить(Поле КАК СТРОКА(10))` or `cast(x as string(10))`
- **THEN** the function name is a keyword token that preserves the original
  lexeme

### Requirement: Recognize conditional and pattern keywords bilingually
The lexer SHALL classify `ЕСТЬNULL`/`ISNULL`, `ПОДОБНО`/`LIKE`, and
`СПЕЦСИМВОЛ`/`ESCAPE` case-insensitively as three keyword kinds whose stable
display names are `ISNULL`, `LIKE`, and `ESCAPE`, and the exhaustive keyword
table test SHALL include all six spellings.

#### Scenario: Mixed-script keyword
- **WHEN** input contains `ЕстьNULL(Поле, 0)` or `isnull(x, 0)`
- **THEN** the function name is one keyword token that preserves the original
  lexeme

#### Scenario: Pattern operator keywords
- **WHEN** input contains `Наименование ПОДОБНО "А%" СПЕЦСИМВОЛ "\"` or its
  English spelling
- **THEN** `ПОДОБНО`/`LIKE` and `СПЕЦСИМВОЛ`/`ESCAPE` are keyword tokens

### Requirement: Recognize temporary-table keywords bilingually
The lexer SHALL classify `ДОБАВИТЬ`/`ADD`, `УНИЧТОЖИТЬ`/`DROP`,
`ИНДЕКСИРОВАТЬ`/`INDEX`, `НАБОРАМ`/`SETS`, and `УНИКАЛЬНО`/`UNIQUE`
case-insensitively as five keyword kinds whose stable display names are
`ADD`, `DROP`, `INDEX`, `SETS`, and `UNIQUE`, and the exhaustive keyword
table test SHALL include all ten spellings. The parser SHALL treat these
keywords as identifiers outside their clauses so that fields and aliases
with those names keep resolving.

#### Scenario: Batch keywords
- **WHEN** input contains `ДОБАВИТЬ ВТ`, `УНИЧТОЖИТЬ ВТ`, and
  `ИНДЕКСИРОВАТЬ ПО НАБОРАМ ((Код) УНИКАЛЬНО)` or their English spellings
- **THEN** each keyword is one token of its kind preserving the original
  lexeme

#### Scenario: Keyword used as a field name
- **WHEN** a query projects a field named `Уникально`
- **THEN** the parser resolves it as a field reference
