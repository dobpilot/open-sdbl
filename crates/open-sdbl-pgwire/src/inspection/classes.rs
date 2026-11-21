//! Platform collection identifiers and the module slots of their members.
//! The collection identifiers can also be cross-checked against v8unpack:
//! <https://github.com/saby-integration/v8unpack/blob/main/src/v8unpack/metadata_types.py>

#[derive(Clone, Copy)]
pub(super) enum Child {
    Form,
    Template,
    Command,
    Descriptor(&'static str),
}

impl Child {
    pub(super) fn from_class(class: &str) -> Option<Self> {
        Some(match class {
            "cf4abea7-37b2-11d4-940f-008048da11f9"
            | "45e46cbc-3e24-4165-8b7b-cc98a6f80211"
            | "ec6bb5e5-b7a8-4d75-bec9-658107a699cf"
            | "7e7123e0-29e2-11d6-a3c7-0050bae0a776"
            | "1a1b4fea-e093-470d-94ff-1d2f16cda2ab"
            | "87c988de-ecbf-413b-87b0-b9516df05e28"
            | "a2207540-1400-11d6-a3c7-0050bae0a776"
            | "b64d9a42-1642-11d6-a3c7-0050bae0a776"
            | "9d28ee33-9c7e-4a1b-8f13-50aa9b36607b" => Self::Descriptor("Реквизиты"),
            "932159f9-95b2-4e76-a8dd-8849fe5c5ded"
            | "21c53e09-8950-4b5e-a6a0-1054f1bbc274"
            | "2bcef0d1-0981-11d6-b9b8-0050bae0a95d"
            | "a3fe6537-d787-40f7-8a06-419d2f0c1cfd" => Self::Descriptor("ТабличныеЧасти"),
            "bee0a08c-07eb-40c0-8544-5c364c171465" => Self::Descriptor("Значения"),
            "13134203-f60b-11d5-a3c7-0050bae0a776"
            | "b64d9a43-1642-11d6-a3c7-0050bae0a776"
            | "35b63b9d-0adf-4625-a047-10ae874c19a3" => Self::Descriptor("Измерения"),
            "13134202-f60b-11d5-a3c7-0050bae0a776"
            | "b64d9a41-1642-11d6-a3c7-0050bae0a776"
            | "63405499-7491-4ce3-ac72-43433cbe4112" => Self::Descriptor("Ресурсы"),
            "3daea016-69b7-4ed4-9453-127911372fe6" => Self::Template,
            "d5b0e5ed-256d-401c-9c36-f630cafd8a62" // Processing
            | "fdf816d2-1ead-11d5-b975-0050bae0a95d" // Catalog
            | "fb880e93-47d7-4127-9357-a20e69c17545" // Document
            | "a3b368c0-29e2-11d6-a3c7-0050bae0a776" // Report
            | "ec81ad10-ca07-11d5-b9a5-0050bae0a95d"
            | "33f2e54b-37ce-4a7a-a569-b648d7aa4634"
            | "b64d9a44-1642-11d6-a3c7-0050bae0a776"
            | "13134204-f60b-11d5-a3c7-0050bae0a776"
            | "d3b5d6eb-4ea2-4610-a3e2-624d4e815934"
            | "a2cb086c-db98-43e4-a1a9-0760ab048f8d"
            | "3f7a8120-b71a-4265-98bf-4d9bc09b7719"
            | "5372e285-03db-4f8c-8565-fe56f1aea40e"
            | "a7f8f92a-7a4b-484b-937e-42d242e64144"
            | "eb2b78a8-40a6-4b7e-b1b3-6ca9966cbc94"
            | "87c509ab-3d38-4d67-b379-aca796298578"
            | "3448e506-5add-4fce-a604-7305466b2d8e"
            | "17816ebc-4068-496e-adc4-8879945a832f"
            | "00867c40-06b1-11d6-a3c7-0050bae0a776"
            | "b8533c0c-2342-4db3-91a2-c2b08cbf6b23"
            | "3f58cbfb-4172-4e54-be49-561a579bb38b" => Self::Form,
            "45556acb-826a-4f73-898a-6025fc9536e1"
            | "4fe87c89-9ad4-43f6-9fdb-9dc83b3879c6"
            | "b544fc6a-2ba3-4885-8fb2-cb289fb6d65e"
            | "e7ff38c0-ec3c-47a0-ae90-20c73ca72246"
            | "7162da60-f7fe-4d78-ad5d-e31700f9af18"
            | "99f328af-a77f-4572-a2d8-80ed20c81890"
            | "7a3e533c-f232-40d5-a932-6a311d2480bf"
            | "acdf0f11-2d59-4e37-9945-c6721871a8fe"
            | "0df30176-6865-4787-9fc8-609eb144174f"
            | "2e90c75b-2f0c-4899-a7d4-5426eaefc96e"
            | "95b5e1d4-abfa-4a16-818d-a5b07b7d3f73"
            | "a49a35ce-120a-4c80-8eea-b0618479cd70"
            | "6d8d73a7-ba29-401d-9032-3872ec2d6433"
            | "d5207c64-11d5-4d46-bba2-55b7b07ff4eb"
            | "4ee40ec7-3469-439f-adb4-aa26ce2d3ec3"
            | "5bb6f09e-5d80-41f6-8070-9faa4d15b69b"
            | "23fa3b84-220a-40e9-8331-e588bed87f7d"
            | "b44ba719-945c-445c-8aab-1088fa4df16e"
            | "f27c2152-a2c9-4c30-adb1-130f5eb2590f" => Self::Command,
            _ => return None,
        })
    }

    pub(super) fn group(self) -> &'static str {
        match self {
            Self::Form => "Формы",
            Self::Template => "Макеты",
            Self::Command => "Команды",
            Self::Descriptor(group) => group,
        }
    }
}

pub(super) struct Class {
    pub(super) name: &'static str,
    pub(super) modules: &'static [(&'static str, &'static str)],
    pub(super) body: Option<Child>,
}

const MANAGER: &str = "МодульМенеджера.bsl";
const OBJECT: &str = "МодульОбъекта.bsl";
const RECORD_SET: &str = "МодульНабораЗаписей.bsl";
const MODULE: &str = "Модуль.bsl";

pub(super) fn classes(id: &str) -> Option<Class> {
    let (name, modules, body): (_, &[(_, _)], _) = match id {
        "cf4abea6-37b2-11d4-940f-008048da11f9" => {
            ("Справочник", &[(".3", MANAGER), (".0", OBJECT)], None)
        }
        "061d872a-5787-460e-95ac-ed74ea3a3e84" => {
            ("Документ", &[(".2", MANAGER), (".0", OBJECT)], None)
        }
        "0195e80c-b157-11d4-9435-004095e12fc7" => {
            ("Константа", &[(".1", MANAGER), (".0", RECORD_SET)], None)
        }
        "13134201-f60b-11d5-a3c7-0050bae0a776" => (
            "РегистрСведений",
            &[(".2", MANAGER), (".1", RECORD_SET)],
            None,
        ),
        "b64d9a40-1642-11d6-a3c7-0050bae0a776" => (
            "РегистрНакопления",
            &[(".2", MANAGER), (".1", RECORD_SET)],
            None,
        ),
        "2deed9b8-0056-4ffe-a473-c20a6c32a0bc" => (
            "РегистрБухгалтерии",
            &[(".7", MANAGER), (".6", RECORD_SET)],
            None,
        ),
        "f2de87a8-64e5-45eb-a22d-b3aedab050e7" => (
            "РегистрРасчета",
            &[(".2", MANAGER), (".1", RECORD_SET)],
            None,
        ),
        "238e7e88-3c5f-48b2-8a3b-81ebbecb20ed" => {
            ("ПланСчетов", &[(".15", MANAGER), (".14", OBJECT)], None)
        }
        "82a1b659-b220-4d94-a9bd-14d757b95a48" => (
            "ПланВидовХарактеристик",
            &[(".16", MANAGER), (".15", OBJECT)],
            None,
        ),
        "30b100d6-b29f-47ac-aec7-cb8ca8a54767" => {
            ("ПланВидовРасчета", &[(".3", MANAGER), (".0", OBJECT)], None)
        }
        "857c4a91-e5f4-4fac-86ec-787626f1c108" => {
            ("ПланОбмена", &[(".3", MANAGER), (".2", OBJECT)], None)
        }
        "4612bd75-71b7-4a5c-8cc5-2b0b65f9fa0d" => ("ЖурналДокументов", &[(".1", MANAGER)], None),
        "631b75a0-29e2-11d6-a3c7-0050bae0a776" => {
            ("Отчет", &[(".2", MANAGER), (".0", OBJECT)], None)
        }
        "bf845118-327b-4682-b5c6-285d2a0eb296" => {
            ("Обработка", &[(".2", MANAGER), (".0", OBJECT)], None)
        }
        "fcd3404e-1523-48ce-9bc0-ecdb822684a1" => {
            ("БизнесПроцесс", &[(".8", MANAGER), (".6", OBJECT)], None)
        }
        "3e63355c-1378-4953-be9b-1deb5fb6bec5" => {
            ("Задача", &[(".7", MANAGER), (".6", OBJECT)], None)
        }
        "46b4cd97-fd13-4eaa-aba2-3bddd7699218" => ("ХранилищеНастроек", &[(".8", MANAGER)], None),
        "0fe48980-252d-11d6-a3c7-0050bae0a776" => ("ОбщийМодуль", &[(".0", MODULE)], None),
        "0fffc09c-8f4c-47cc-b41c-8d5c5a221d79" => ("HTTPСервис", &[(".0", MODULE)], None),
        "8657032e-7740-4e1d-a3ba-5dd6e8afb78f" => ("WebСервис", &[(".0", MODULE)], None),
        "07ee8426-87f1-11d5-b99c-0050bae0a95d" => ("ОбщаяФорма", &[], Some(Child::Form)),
        "0c89c792-16c3-11d5-b96b-0050bae0a95d" => ("ОбщийМакет", &[], Some(Child::Template)),
        "2f1a5187-fb0e-4b05-9489-dc5dd6412348" => ("ОбщаяКоманда", &[], Some(Child::Command)),
        "f6a80749-5ad7-400b-8519-39dc5dff2542" => ("Перечисление", &[], None),
        "09736b02-9cac-4e3f-b4f7-d3e9576ab948" => ("Роль", &[], None),
        "37f2fa9a-b276-11d4-9435-004095e12fc7" => ("Подсистема", &[], None),
        "9cd510ce-abfc-11d4-9434-004095e12fc7" => ("Язык", &[], None),
        "24c43748-c938-45d0-8d14-01424a72b11e" => ("ПараметрСеанса", &[], None),
        "af547940-3268-434f-a3e7-e47d6d2638c3" => ("ФункциональнаяОпция", &[], None),
        "30d554db-541e-4f62-8970-a1c6dcfeb2bc" => ("ПараметрФункциональныхОпций", &[], None),
        "11bdaf85-d5ad-4d91-bb24-aa0eee139052" => ("РегламентноеЗадание", &[], None),
        "4e828da6-0f44-4b5b-b1c0-a2b3cfe7bdcc" => ("ПодпискаНаСобытие", &[], None),
        "15794563-ccec-41f6-a83c-ec5f7b9a5bc1" => ("ОбщийРеквизит", &[], None),
        "7dcd43d9-aca5-4926-b549-1842e6a4e8cf" => ("ОбщаяКартинка", &[], None),
        "c045099e-13b9-4fb6-9d50-fca00202971e" => ("ОпределяемыйТип", &[], None),
        "39bddf6a-0c3c-452b-921c-d99cfa1c2f1b" => ("Интерфейс", &[], None),
        "1c57eabe-7349-44b3-b1de-ebfeab67b47d" => ("ГруппаКоманд", &[], None),
        "36a8e346-9aaa-4af9-bdbd-83be3c177977" => ("Нумератор", &[], None),
        "bc587f20-35d9-11d6-a3c7-0050bae0a776" => ("Последовательность", &[], None),
        "3e7bfcc0-067d-11d6-a3c7-0050bae0a776" => ("КритерийОтбора", &[], None),
        "3e5404af-6ef8-4c73-ad11-91bd2dfac4c8" => ("Стиль", &[], None),
        "58848766-36ea-4076-8800-e91eb49590d7" => ("ЭлементСтиля", &[], None),
        "5274d9fc-9c3a-4a71-8f5e-a0db8ab23de5" => ("ВнешнийИсточникДанных", &[], None),
        "6e6dc072-b7ac-41e7-8f88-278d25b6da2a" => ("Бот", &[], None),
        "bf3420b0-f6f9-41a0-b83a-fe9d4ab0b65d" => ("СервисИнтеграции", &[], None),
        "a7641777-7813-45c6-96ef-9d51587a6ac6" => ("WebSocketКлиент", &[], None),
        "d26096fb-7a5d-4df9-af63-47d04771fa9b" => ("WSСсылка", &[], None),
        "cc9df798-7c94-4616-97d2-7aa0b7bc515e" => ("ПакетXDTO", &[], None),
        _ => return None,
    };
    Some(Class {
        name,
        modules,
        body,
    })
}
