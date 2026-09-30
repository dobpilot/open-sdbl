# Design

Native XML and acquired Config are matched by UUID for 296 UNF catalogs. Position 37 of class 57 is 0 for all 219 Hierarchical=false catalogs and 1 for all 77 Hierarchical=true catalogs. No disagreements. Earlier positions 10 through 16 do not represent this property and are not used.

Add only ConfigDescriptor.catalog_hierarchical: Option<bool> on the catalog owner. Reuse the existing recognized resource version, generated UUID prefix and own header checks; retain only measured scalar 0/1 at position 37 in the streaming parser. Missing, quoted or unknown values remain None. Nested descriptors retain None. No hierarchy kind, parent value semantics or saving policy is inferred or implemented here.
