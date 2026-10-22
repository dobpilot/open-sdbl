Процедура М(Ид, Значение)
 Сообщить(Ид + Символ(9) + Строка(Значение));
КонецПроцедуры
М("file.begin", "functional-option-content-1");
Попытка
 ОпцияЛожь = Метаданные.ФункциональныеОпции.ОпцияПробы;
 ОпцияИстина = Метаданные.ФункциональныеОпции.ОпцияПробыПривилегированная;
 М("FUNCTIONAL_OPTION.CONTENT.FALSE_COUNT", ОпцияЛожь.Состав.Количество());
 М("FUNCTIONAL_OPTION.CONTENT.TRUE_COUNT", ОпцияИстина.Состав.Количество());
 М("FUNCTIONAL_OPTION.CONTENT.FALSE_LOCATION", ОпцияЛожь.Хранение.ПолноеИмя());
 М("FUNCTIONAL_OPTION.CONTENT.TRUE_LOCATION", ОпцияИстина.Хранение.ПолноеИмя());
 М("FUNCTIONAL_OPTION.CONTENT.FALSE_MODE", ОпцияЛожь.ПривилегированныйРежимПриПолучении);
 М("FUNCTIONAL_OPTION.CONTENT.TRUE_MODE", ОпцияИстина.ПривилегированныйРежимПриПолучении);
Исключение
 М("FUNCTIONAL_OPTION.CONTENT.ERROR", ОписаниеОшибки());
КонецПопытки;
М("file.end", "functional-option-content-1");
