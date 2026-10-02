# Design

Expose `DocumentNumberPeriodicity`, `DocumentNumberingSettings { periodicity, numerator: Option<Guid> }` and `ConfigDescriptor::document_numbering: Option<DocumentNumberingSettings>`. The outer Option means acquisition completeness: any unknown class shape, periodicity value or malformed numerator makes the whole settings value None. Inside an acquired value, numerator None means a measured nil object reference, not an unknown value. The existing Copy DocumentNumberSettings remains unchanged.

The native UNF XML and matching Config establish document class40 length53, numerator at class[10] and periodicity at class[13]. Compare all125 documents and exact numerator object UUIDs, including the two documents sharing the same numerator. Supplement missing enum mappings with a separate native minimum configuration, import/apply/export and matching Config acquisition. Only measured mappings are decoded. Unknown codes are not rounded or given defaults.

Acquiring configured properties does not establish number reservation scope, lifetime, shared-numerator runtime behavior, ordering, uniqueness or SQL allocation. These belong to separate measured runtime work. The decoder stays pure and transport-independent.
