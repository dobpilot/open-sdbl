## ADDED Requirements

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
