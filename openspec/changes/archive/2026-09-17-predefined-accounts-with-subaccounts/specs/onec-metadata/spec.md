## ADDED Requirements

### Requirement: Predefined accounts with subaccounts
A row of a predefined-item resource whose element count exceeds the
column count plus the header and trailer — an account with subaccounts
in a `.9` resource — SHALL yield its predefined value like a leaf row,
and the rows nested in its trailer SHALL be read as well.

#### Scenario: Account 50 with subaccounts
- **WHEN** the `.9` resource holds `{2,124,13,{"#",T,{1,G}},{"S","Касса"},…,1,{1,6,{2,125,13,…}}}`
- **THEN** `Касса` and the nested accounts resolve as predefined values
