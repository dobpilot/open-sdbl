## ADDED Requirements

### Requirement: Complete qualified virtual-table sources
The interactive console SHALL derive virtual-table completion candidates from
the resolved metadata kind. It SHALL offer Russian and English virtual-table
names after bare, Russian-kind-qualified, and English-kind-qualified register
object spellings, including an empty argument list accepted by the parser. It
SHALL NOT offer register virtual tables for unrelated metadata kinds.

#### Scenario: Accumulation-register virtual completion
- **WHEN** Tab follows a partial qualified accumulation-register source
- **THEN** completion offers `Остатки()`/`Balance()` and
  `Обороты()`/`Turnovers()` candidates for that object

#### Scenario: Information-register virtual completion
- **WHEN** Tab follows a partial qualified information-register source
- **THEN** completion offers `СрезПоследних()`/`SliceLast()` and
  `СрезПервых()`/`SliceFirst()` candidates for that object

#### Scenario: Non-register object completion
- **WHEN** completion candidates are built for a catalog or another unrelated
  metadata kind
- **THEN** no register virtual-table suffix is attached to that object
