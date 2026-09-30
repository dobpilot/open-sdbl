# Иерархичность справочника из Config

2026-10-01. Native XML 1С 8.3.27.2342 УНФ сопоставлена по UUID с 296 приобретёнными Config descriptors: 219 Hierarchical=false соответствуют 0, 77 Hierarchical=true соответствуют 1 в позиции 37 класса 57. Ранние позиции 10–16 этому флагу не соответствуют и не использованы. Эталон сохранён в tests/fixtures/catalog-hierarchy/unf-native.tsv; никаких SQL/source эвристик.

API: ConfigDescriptor.catalog_hierarchical: Option<bool> только у owner. Проверки resource version/prefix/header переиспользуются; отсутствующее, строковое, неизвестное значение и непонятная форма дают None. Nested descriptors не наследуют флаг. Код приобретает метаданные и не определяет Parent getter/setter, вид иерархии или правила save. Версия остаётся 0.9.5.

Компактные fixtures сохраняют нативный class prefix до позиции флага; полный corpus проверяется отдельно. Старые ожидания тестов не менялись, литералы лишь получили None. Живые базы в этой работе не изменялись и не читались.
