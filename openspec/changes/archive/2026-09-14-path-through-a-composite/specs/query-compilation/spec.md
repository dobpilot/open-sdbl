## ADDED Requirements

### Requirement: A reference path continuing past a composite hop
A reference path SHALL continue past a hop whose field may point at
several tables: every target SHALL be joined under a guard on the stored
type, the rest of the path SHALL be walked inside that target, and the
branches SHALL be selected by the same type. A target in which the rest of
the path does not resolve SHALL contribute no branch. A hop through a
field that is not a reference at all SHALL keep reporting that.

#### Scenario: Two hops past a composite field
- **WHEN** `ВЫБРАТЬ Д.Составное.Ссылка.Ссылка ИЗ Документ.X КАК Д` is
  compiled
- **THEN** each target of the composite is joined under its type guard and
  the next hop is joined from that target

#### Scenario: A target without the rest of the path
- **WHEN** one target of the composite does not define the next field
- **THEN** that target contributes no branch and the query still compiles

#### Scenario: A hop through a value that is not a reference
- **WHEN** the path continues through a string field
- **THEN** the query is refused, naming the field
