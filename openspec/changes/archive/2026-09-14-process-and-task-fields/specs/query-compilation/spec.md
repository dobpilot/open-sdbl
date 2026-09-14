## ADDED Requirements

### Requirement: Standard fields of business processes and tasks
A business process SHALL expose `Completed` / `Завершен`, `Started` /
`Стартован` and `HeadTask` / `ВедущаяЗадача`; a task SHALL expose `Name` /
`Наименование`, `Executed` / `Выполнена`, `BusinessProcess` /
`БизнесПроцесс` and `Point` / `ТочкаМаршрута`, next to the reference,
date, number and deletion mark they already carry.

#### Scenario: Task list
- **WHEN** `ВЫБРАТЬ З.Наименование, З.Выполнена, З.БизнесПроцесс ИЗ
  Задача.X КАК З` is compiled
- **THEN** each name reads its column of the task table

#### Scenario: Business process state
- **WHEN** `ГДЕ Б.Завершен` filters a business process
- **THEN** the predicate reads the `Completed` column
