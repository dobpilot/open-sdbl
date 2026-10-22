## ADDED Requirements

### Requirement: Complete measured legacy business-process attributes
For the measured original BUH BusinessProcess descriptor with homogeneous 2/5 attribute wrappers and supported 27/23 bodies, decoding SHALL expose the complete ordered attribute membership confirmed by native XML. Existing complete-type, uniqueness and count checks SHALL remain required. Unknown or mixed wrapper forms SHALL return unknown membership rather than a partial list. Independent owner and section metadata SHALL remain independently available.

#### Scenario: Original native owner
- **WHEN** the retained original BusinessProcess Задание descriptor is decoded
- **THEN** all 27 UUID/name/type entries match the native XML order, including Автор

#### Scenario: Unrecognized member
- **WHEN** any row has an unknown marker/length, mixed wrapper shape, duplicate identity or missing recognized type
- **THEN** the entire attribute collection is unknown and no shortened list is exposed
