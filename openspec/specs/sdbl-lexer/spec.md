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

The library SHALL stop at malformed strings, parameters, or unsupported
characters and return a diagnostic with its source position.

#### Scenario: Unterminated string

- **WHEN** the input ends inside a string literal
- **THEN** tokenization returns an unterminated-string diagnostic at the
  opening quote

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
