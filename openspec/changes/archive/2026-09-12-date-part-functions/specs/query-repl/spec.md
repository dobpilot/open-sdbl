## ADDED Requirements

### Requirement: Extract date parts
The compiler SHALL accept `ГОД`, `КВАРТАЛ`, `МЕСЯЦ`, `ДЕНЬГОДА`, `ДЕНЬ`,
`НЕДЕЛЯ`, `ДЕНЬНЕДЕЛИ`, `ЧАС`, `МИНУТА`, `СЕКУНДА` and their English
spellings with one date field or date-kind expression and SHALL return an
integer number on PostgreSQL and MSSQL: the calendar year, quarter (1–4),
month, day of year, day of month, hour, minute, and second; `ДЕНЬНЕДЕЛИ`
SHALL be 1 for Monday through 7 for Sunday regardless of the server's
first-weekday setting; `НЕДЕЛЯ` SHALL number weeks as the platform does:
the week containing 1 January is week 1, weeks start on Monday, and
numbering restarts on 1 January. On MSSQL with a non-zero year offset the
parts SHALL be taken from the logical date. A non-date argument SHALL be a
`Syntax` diagnostic. The functions SHALL nest with the other date
functions and SHALL be accepted as `GROUP BY` keys.

#### Scenario: Grouping by year
- **WHEN** `ВЫБРАТЬ ГОД(Т.Дата) КАК Год, КОЛИЧЕСТВО(*) КАК Н ИЗ … СГРУППИРОВАТЬ ПО ГОД(Т.Дата)`
  is compiled
- **THEN** generated SQL projects and groups by the year expression and the
  column kind is number

#### Scenario: Platform week numbering
- **WHEN** `НЕДЕЛЯ(Дата)` is applied to 2021-01-03, 2021-01-04, 2024-12-30,
  and 2025-01-01
- **THEN** the results are 1, 2, 53, and 1 on both providers

#### Scenario: Weekday under a year offset
- **WHEN** `ДЕНЬНЕДЕЛИ(Дата)` and `ГОД(Дата)` are compiled for MSSQL with
  year offset 2000
- **THEN** generated SQL applies `DATEADD(year, -2000, …)` to the column
  before taking the parts
