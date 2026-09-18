## ADDED Requirements

### Requirement: Decode chart-of-accounts extra-dimension metadata
The decoder SHALL expose, for an accounting register, the chart of accounts
it is bound to and whether it keeps correspondence, and, for a chart of
accounts, the maximum number of extra dimensions and the physical table
that lists the extra-dimension kinds of each account.

#### Scenario: Register bound to a chart
- **WHEN** a register names a chart of accounts with two extra dimensions
- **THEN** the resolved register carries the chart's identity, and the
  chart carries the count `2` and its extra-dimension kinds table
