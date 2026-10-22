# Native legacy reporting-directions textual owner comment

The original BUH catalog НаправленияСдачиОтчетности (b5697377-16f3-4e8c-b65b-1742fbe55dd0) cannot resolve ТипНаправления at module line 17 on the baseline SDK. Paired native CF/XML establishes class 53/58, strict owner identity 1/9 and four own attributes with exact ordered qualified types. The owner has nonempty String comment `Сведения о доступных направлениях сдачи отчетности`, equal to XML Comment. Child comments are empty, flags and all other measured profile fields remain unchanged.

Four own XML declarations: Организация String12Variable, ТипНаправления EnumRef.ТипыНаправленийСдачиОтчетности (8a20267d-5c6a-451a-a8a6-2774b0761f66), Направление String7Variable, ДатаАктуализации DateTime. The native section collection is explicitly empty. External native Enum GeneratedType evidence and all 10 generated owner TypeId/ValueId locations are retained.

The causal control synthetic-empty-comment.deflate is explicitly synthetic diagnostic data: its tree differs from original CF only at owner comment [1,9,1,4]. The unmodified prior public SDK then restores all four exact XML declarations and known empty sections. Original raw CF/XML bytes remain retained unchanged. Causal controls and previous inspection logs describe baseline SDK behavior, not post-fix state.

The implementation changes only the local owner-comment check from empty String to String-kind acceptance. Shared headers, child comments, owner flags, settings, runtime/public API and modern paths are unchanged. New external tests retain native red/green, the already supported empty String positive, Atom(empty/nonempty), Null, List(empty/nonempty) negative cases, and existing owner-flag/child-comment guard controls. Existing test files remain unchanged.

Version remains 0.9.20, without release, commit or archive. No live database acquisition or write was used.

Independent public verification confirms all four exact native declarations and known empty sections. The complete 752-catalog comparison changes exactly this owner (four own attributes); 751 rows remain unchanged, including all 727 modern non53 full descriptor projections. Full before/after tables, manifest and verifier receipts are retained.
