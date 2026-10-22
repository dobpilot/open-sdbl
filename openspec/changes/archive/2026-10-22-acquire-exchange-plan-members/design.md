# Design

Native 1C 8.3.27.2342 DEMO Config/Designer XML and EDT describe ExchangePlan ИнтегрированныеСистемы (d4becac6-bf5f-4947-a734-c702d399d5fc) with eight own attributes. Its owner class is 37 with 51 fields, generated GUID slots 1 through 11, and own identity at slot 12. This differs from legacy Document class 37/50. Attribute collection 1a1b4fea-e093-470d-94ff-1d2f16cda2ab contains wrappers 4/7 and bodies 27/23. Reuse existing attribute_bodies and complete_attributes, checking identity, exact owner shape, count, duplicate membership, and acquired typed descriptors. Return None for any unsupported or incomplete form; Some(empty) only for a verified zero collection.

The supplied native ExchangePlans have no tabular sections. Their empty collections do not measure nonempty section row layout, so this change does not expose or infer section membership. No object/runtime operations are added.
