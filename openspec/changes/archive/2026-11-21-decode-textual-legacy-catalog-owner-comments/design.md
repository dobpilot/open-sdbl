# Design

The retained native catalog class 53/58, owner identity 1/9, comments, children and qualified declarations are independently compared with XML. The owner comment at [1,9,1,4] is String `Сведения о доступных направлениях сдачи отчетности`. All four child comments are empty; no child header change is needed. The native section collection is explicitly empty.

A public-API causal control changes only that owner comment to empty String in clearly marked synthetic diagnostic data. The identical prior SDK then restores all four ordered declarations and the known empty section collection. All other native owner and child guards pass.

Replace only the local owner-comment emptiness requirement with the existing String-kind check. Do not change shared headers, child comments, flags, modern settings, runtime API or public metadata fields. Non-string Atom(empty/nonempty), Null and List comments remain unrecognized. New external tests retain the full native pair, exact UUID/name/order/types/references/qualifiers, a positive empty-comment control and malformed comment variants. Existing tests remain unchanged.
