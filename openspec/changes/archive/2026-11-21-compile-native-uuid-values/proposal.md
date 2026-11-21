# Change: Compile typed native UUID values

## Why
Original demo and UNF session initialization pass an InfoBaseUser UUID to a UNION ALL query. Treating UUID as bytes loses both parameter identity and result typing. Own native PostgreSQL fixtures establish the declaration and physical byte order.

## What Changes
- Add a typed UUID parameter using the existing Guid.
- Recognize unambiguous singleton UUID metadata attributes and convert their physical binary storage through the existing dialect UUID conversion.
- Preserve UUID columns across projections, subqueries and UNION ALL.
- Reject the newly supported forms on unmeasured providers; retain existing UUID(reference) behavior.

## Impact
Query parameter API, metadata query classification and PostgreSQL query compilation. No dependencies, database writes, release version or main checkout changes.
