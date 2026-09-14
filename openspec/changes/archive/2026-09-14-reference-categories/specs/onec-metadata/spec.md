## ADDED Requirements

### Requirement: Reference categories in a type description
A reference type that names a kind rather than one object SHALL resolve to
every object of that kind, and the category of every reference SHALL
resolve to every reference object of the configuration. A description
mixing such a category with named objects SHALL resolve to the union.

#### Scenario: A field typed as any business process
- **WHEN** a field whose type description names the business-process
  category is dereferenced
- **THEN** every business process of the configuration is joined under its
  own type guard
