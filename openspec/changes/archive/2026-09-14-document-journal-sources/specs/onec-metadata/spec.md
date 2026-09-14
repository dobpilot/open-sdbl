## ADDED Requirements

### Requirement: Document journals are a resolved kind
`DocumentJournal` SHALL be a resolved metadata kind: its DBNames alias is
`DocumentJournal` and its physical tables carry the `_DocumentJournal`
prefix.

#### Scenario: Journal in DBNames
- **WHEN** DBNames declares an object under the `DocumentJournal` alias
- **THEN** the snapshot resolves it as a document journal bound to its
  `_DocumentJournal<n>` table
