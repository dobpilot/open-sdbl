# Expose native generic reference type sets

## Why
Runtime declarations expose generic family GUIDs as opaque references even though the SQL resolver already recognizes their measured meanings. A lazy snapshot cannot safely expand a family using only its reached objects.

## What Changes
Expose ReferenceTypeSet with Kind(MetadataKind) and Any, ConfigDeclaredType::reference_type_set(), and ReferenceTypeSet::contains_kind(). Reuse the existing measured native mapping without changing declarations, normalization, SQL, rights or fingerprints. Release version 0.9.41.

## Impact
Additive core Rust API. Consumers expand explicitly against their complete reference catalog, preserving unknown states and deduplicating overlaps. No dependencies or I/O.
