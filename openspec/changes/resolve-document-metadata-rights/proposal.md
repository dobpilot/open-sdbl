# Resolve measured Document metadata rights

## Why
The acquired Document metadata and native 432-call controls distinguish owner rights, standard Number/Date rights and ordinary-own-attribute rights. The current Catalog-only structural resolver cannot express those targets.

## What Changes
Extend the existing owner/standard resolver for Document Read/View and add a separate owner-bound ordinary-attribute resolver. Evaluate retained explicit/default/independent profiles through the existing role cache. Preserve unsupported errors and Catalog behavior.

## Impact
Public Rust API and metadata applicability/evaluation. No SQL/RLS field authorization changes, dependencies, writes, sessions or host-side evaluator.
