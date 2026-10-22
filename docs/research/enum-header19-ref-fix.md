# Исправление EnumRef старого header19

Это bug fix существующего OpenSpec requirement `Enumeration reference identities`
в `openspec/specs/onec-metadata/spec.md`: измеренный EnumRef TypeId должен
приобретаться, а Manager TypeId запрещено подставлять вместо него. API и версия
не меняются; новый change не нужен, поскольку ожидаемое поведение уже задано.
Активные изменения session-startup, report metadata и accounting virtual tables
не изменяются и не архивируются.

Первый тест `unchanged_header19_enum_matches_native_ref_and_survives_snapshot_resolution`
воспроизвёл подмену Manager вместо Ref до изменения кода. Красная квитанция
сохранена в `/tmp/sdbl-enum-header19-red.log`. После исправления приобретается
Ref из нативно измеренной enum-формы header19; снимок сохраняет идентичность.
Пять ресурсов получили независимые нативные оракулы, их байты не менялись.

Потоковый декодер откладывает определение Ref класса19 до полной проекции:
число встречается у старого Enum и современного Report. Проекция проверяет
измеренный профиль целиком, включая старый self-header, GUID-блок, поля и
коллекции с точными счетчиками и уникальными значениями. Только эта форма
публикует class[1]. Современный Report проходит собственную прежнюю проекцию;
header20 сохраняет существующий путь. Неизвестные и повреждённые формы класса19
не публикуют Manager как Ref. Остальные классы не менялись.

Старые тесты и SQL/diagnostic expectations не редактируются. Новые проверки
охватывают все пять exact native fixtures, сохранение Ref после resolution,
modern header20 и Report, неверные GUID, счетчики, заголовки и повторные значения.
Артефакты — `tests/fixtures/enum-header19-native/`; руководство к ним описывает
реальный native LoadCfg/XML round trip и границу измерения.

Проверки прошли: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets --offline -- -D warnings`,
`RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --offline`,
`cargo test --workspace --offline -j 2`, `openspec validate --all --strict`.
Для локальных сетевых unit-тестов workspace разрешены localhost-сокеты;
исходные БД не использовались. Build profile имеет DEBUG=0, INCREMENTAL=0,
CCACHE_DIR=/tmp/open-sdbl-ccache. Подробные журналы:
`/tmp/sdbl-enum-header19-{red,green,workspace,clippy,rustdoc}.log`.
Новых зависимостей нет; версия0.9.15 и активные change-пакеты не изменены.
