# Design

The paired native Config and Designer XML establish class 32/56, owner identity 1/9 at class position 13/1, value Pattern at 18 and code/description lengths at 21/23 (20 and 50). Its five attributes use homogeneous row wrappers 1/5, bodies 27/23, typed declarations 2/3 and identity 1/7. Validate the whole local profile before publishing attribute declarations or membership. Reuse existing declaration, collection, identity and filling helpers; do not weaken global header or modern membership checks.

The stream scanner must select class 32 for the existing tree projection; changing only the runtime guard would be ineffective. Generated reference identity already belongs to the existing scanner contract. Tests compare its identity with XML and distinguish the raw generated object identity without adding an API for it. Unknown reference syntax is rejected; valid reference identities remain raw acquired metadata, not guessed targets.

The previous conversion review copy and all source checkouts stay frozen. This isolated review keeps version 0.9.20, makes no commits or release, changes no old test expectations and is not archived while the inherited UUID SQL corpus gate fails.
