//! Roles of a configuration: which roles it declares, the rights every
//! role grants on metadata objects, and the row-level restriction texts and
//! templates those rights carry.
//!
//! A role is a bare-GUID Config resource (its descriptor, which names it)
//! plus the resource `<guid>.0` holding its rights in the brace format:
//!
//! ```text
//! {10, {N, {{1, <объект>, K, <члены…>, F}, {0, <право>, <значение>, …}}, …},
//!      {T, {"<Имя(параметры)>", "<тело>"}, …}, <флаги…>}
//! ```
//!
//! A rights list opening with `1` carries the count of its pairs, then a
//! count of restriction nodes `{<право>, {M, {1, "<условие>", L, {<поля>}}, …}}`.
//! The value `1` grants a right, `-1` records an explicit refusal.

use std::str::FromStr;

use super::config::ConfigDescriptor;
use super::value::{Value, parse_serialized};
use super::{Guid, MetadataError, MetadataErrorKind, inflate_raw_deflate_bounded};

/// The collection of the configuration root that lists the roles.
pub const ROLES_COLLECTION: &str = "09736b02-9cac-4e3f-b4f7-d3e9576ab948";

/// The format version of a rights resource this module decodes.
const RIGHTS_FORMAT_VERSION: u32 = 10;

/// A right the platform grants on a metadata object, named as the
/// platform names it in an XML export.
///
/// The identifiers are fixed platform GUIDs; the names follow from the
/// order the rights resource writes them in — the order of the role editor
/// — measured on 8.3.27 against БП 3.0 and УНФ and confirmed by the
/// single-right roles of БСП. An identifier the table does not list is
/// carried as [`Right::Other`].
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Right {
    /// Чтение.
    Read,
    /// Добавление.
    Insert,
    /// Изменение.
    Update,
    /// Удаление.
    Delete,
    /// Просмотр.
    View,
    /// Интерактивное добавление.
    InteractiveInsert,
    /// Редактирование.
    Edit,
    /// Интерактивное удаление.
    InteractiveDelete,
    /// Интерактивная пометка удаления.
    InteractiveSetDeletionMark,
    /// Интерактивное снятие пометки удаления.
    InteractiveClearDeletionMark,
    /// Интерактивное удаление помеченных.
    InteractiveDeleteMarked,
    /// Ввод по строке.
    InputByString,
    /// Интерактивное удаление предопределённых данных.
    InteractiveDeletePredefinedData,
    /// Интерактивная пометка удаления предопределённых данных.
    InteractiveSetDeletionMarkPredefinedData,
    /// Интерактивное снятие пометки удаления предопределённых данных.
    InteractiveClearDeletionMarkPredefinedData,
    /// Интерактивное удаление помеченных предопределённых данных.
    InteractiveDeleteMarkedPredefinedData,
    /// Чтение истории данных.
    ReadDataHistory,
    /// Изменение истории данных.
    UpdateDataHistory,
    /// Просмотр истории данных.
    ViewDataHistory,
    /// Редактирование истории данных.
    EditDataHistory,
    /// Изменение настроек истории данных.
    UpdateDataHistorySettings,
    /// Изменение комментария версии истории данных.
    UpdateDataHistoryVersionComment,
    /// Просмотр настроек истории данных.
    ViewDataHistorySettings,
    /// Редактирование комментария версии истории данных.
    EditDataHistoryVersionComment,
    /// Переход на версию истории данных.
    SwitchToDataHistoryVersion,
    /// Проведение.
    Posting,
    /// Отмена проведения.
    UndoPosting,
    /// Интерактивное проведение.
    InteractivePosting,
    /// Интерактивное проведение неоперативное.
    InteractivePostingRegular,
    /// Интерактивная отмена проведения.
    InteractiveUndoPosting,
    /// Интерактивное изменение проведённых.
    InteractiveChangeOfPosted,
    /// Управление итогами.
    TotalsControl,
    /// Использование.
    Use,
    /// Интерактивная активация.
    InteractiveActivate,
    /// Старт.
    Start,
    /// Интерактивный старт.
    InteractiveStart,
    /// Выполнение.
    Execute,
    /// Интерактивное выполнение.
    InteractiveExecute,
    /// Получение (параметр сеанса).
    Get,
    /// Установка (параметр сеанса).
    Set,
    /// Администрирование.
    Administration,
    /// Администрирование данных.
    DataAdministration,
    /// Обновление конфигурации базы данных.
    UpdateDataBaseConfiguration,
    /// Монопольный режим.
    ExclusiveMode,
    /// Активные пользователи.
    ActiveUsers,
    /// Журнал регистрации.
    EventLog,
    /// Тонкий клиент.
    ThinClient,
    /// Веб-клиент.
    WebClient,
    /// Мобильный клиент.
    MobileClient,
    /// Толстый клиент.
    ThickClient,
    /// Внешнее соединение.
    ExternalConnection,
    /// Automation.
    Automation,
    /// Режим технического специалиста.
    TechnicalSpecialistMode,
    /// Регистрация информационной базы в системе взаимодействия.
    CollaborationSystemInfoBaseRegistration,
    /// Режим основного окна: обычный.
    MainWindowModeNormal,
    /// Режим основного окна: рабочее место.
    MainWindowModeWorkplace,
    /// Режим основного окна: встроенное рабочее место.
    MainWindowModeEmbeddedWorkplace,
    /// Режим основного окна: полноэкранное рабочее место.
    MainWindowModeFullscreenWorkplace,
    /// Режим основного окна: киоск.
    MainWindowModeKiosk,
    /// Клиент аналитической системы.
    AnalyticsSystemClient,
    /// Сохранение данных пользователя.
    SaveUserData,
    /// Администрирование расширений конфигурации.
    ConfigurationExtensionsAdministration,
    /// Интерактивное открытие внешних обработок.
    InteractiveOpenExtDataProcessors,
    /// Интерактивное открытие внешних отчётов.
    InteractiveOpenExtReports,
    /// Вывод.
    Output,
    /// An identifier the table does not name.
    Other(Guid),
}

/// The right identifiers with their names and Russian synonyms.
const RIGHTS: &[(&str, Right, &str, &str)] = &[
    (
        "1c87578f-9e09-4ec0-a991-5629c87b1588",
        Right::Read,
        "Read",
        "Чтение",
    ),
    (
        "33200740-82b0-4de7-8556-d3fb25ca4328",
        Right::Insert,
        "Insert",
        "Добавление",
    ),
    (
        "287b74b8-3a66-4a76-ba27-4f1f6a93770e",
        Right::Update,
        "Update",
        "Изменение",
    ),
    (
        "c0028105-4cc1-41ca-aef1-bfbd8fc8f8c4",
        Right::Delete,
        "Delete",
        "Удаление",
    ),
    (
        "aa6448f2-be0f-42ea-ba26-1af7f52b5b65",
        Right::View,
        "View",
        "Просмотр",
    ),
    (
        "fb88c756-91c9-4351-9cdf-e027879886c6",
        Right::InteractiveInsert,
        "InteractiveInsert",
        "ИнтерактивноеДобавление",
    ),
    (
        "b7bab52d-c1b1-4bd8-8276-02db08d42352",
        Right::Edit,
        "Edit",
        "Редактирование",
    ),
    (
        "b53db6ed-6e5b-4035-8d24-f10083d646ed",
        Right::InteractiveDelete,
        "InteractiveDelete",
        "ИнтерактивноеУдаление",
    ),
    (
        "d76b72ba-5388-4b7f-af64-1b351f63a1e1",
        Right::InteractiveSetDeletionMark,
        "InteractiveSetDeletionMark",
        "ИнтерактивнаяПометкаУдаления",
    ),
    (
        "798cf688-ad74-44fe-a464-236b49e910e0",
        Right::InteractiveClearDeletionMark,
        "InteractiveClearDeletionMark",
        "ИнтерактивноеСнятиеПометкиУдаления",
    ),
    (
        "fa6dbe86-856a-4ac4-b8ac-bce99f8b8b22",
        Right::InteractiveDeleteMarked,
        "InteractiveDeleteMarked",
        "ИнтерактивноеУдалениеПомеченных",
    ),
    (
        "b5f861d3-d9c5-45ec-98bf-0ed4d489a351",
        Right::InputByString,
        "InputByString",
        "ВводПоСтроке",
    ),
    (
        "013a262e-165f-4815-bdae-7a1bed6a68e4",
        Right::InteractiveDeletePredefinedData,
        "InteractiveDeletePredefinedData",
        "ИнтерактивноеУдалениеПредопределенныхДанных",
    ),
    (
        "408c56c0-e210-4e2e-8e82-610050a08a39",
        Right::InteractiveSetDeletionMarkPredefinedData,
        "InteractiveSetDeletionMarkPredefinedData",
        "ИнтерактивнаяПометкаУдаленияПредопределенныхДанных",
    ),
    (
        "e7f9daf9-eac2-4ada-9c26-c380858f3589",
        Right::InteractiveClearDeletionMarkPredefinedData,
        "InteractiveClearDeletionMarkPredefinedData",
        "ИнтерактивноеСнятиеПометкиУдаленияПредопределенныхДанных",
    ),
    (
        "65e5f92c-40ff-4130-9652-c0e7612d0609",
        Right::InteractiveDeleteMarkedPredefinedData,
        "InteractiveDeleteMarkedPredefinedData",
        "ИнтерактивноеУдалениеПомеченныхПредопределенныхДанных",
    ),
    (
        "64319ca1-f3d8-472e-82ce-5da233e6daaa",
        Right::ReadDataHistory,
        "ReadDataHistory",
        "ЧтениеИсторииДанных",
    ),
    (
        "1b762bf9-df7f-4255-bbe6-f7578f41368d",
        Right::UpdateDataHistory,
        "UpdateDataHistory",
        "ИзменениеИсторииДанных",
    ),
    (
        "b162ff57-0296-483e-9af8-dc37576802cb",
        Right::ViewDataHistory,
        "ViewDataHistory",
        "ПросмотрИсторииДанных",
    ),
    (
        "c4ab1331-e58d-4a46-ad2e-fe6d80b72aa4",
        Right::EditDataHistory,
        "EditDataHistory",
        "РедактированиеИсторииДанных",
    ),
    (
        "a679c969-8ea1-4b8b-9e61-8a414ba448f4",
        Right::UpdateDataHistorySettings,
        "UpdateDataHistorySettings",
        "ИзменениеНастроекИсторииДанных",
    ),
    (
        "5b3ea0e2-fdb9-41f6-bf6c-25747906b4cb",
        Right::UpdateDataHistoryVersionComment,
        "UpdateDataHistoryVersionComment",
        "ИзменениеКомментарияВерсииИсторииДанных",
    ),
    (
        "9342b152-a7ae-4c79-9b7b-f4f028a36479",
        Right::ViewDataHistorySettings,
        "ViewDataHistorySettings",
        "ПросмотрНастроекИсторииДанных",
    ),
    (
        "8497054a-ffd1-4ca7-bdfe-340b9ddc050a",
        Right::EditDataHistoryVersionComment,
        "EditDataHistoryVersionComment",
        "РедактированиеКомментарияВерсииИсторииДанных",
    ),
    (
        "479a42c0-c3e9-4ae7-bf4a-75cebc14fec4",
        Right::SwitchToDataHistoryVersion,
        "SwitchToDataHistoryVersion",
        "ПереходНаВерсиюИсторииДанных",
    ),
    (
        "e060de25-bffd-42fd-bb09-f3a788d65760",
        Right::Posting,
        "Posting",
        "Проведение",
    ),
    (
        "f55a8f7f-2c65-404f-b530-093d9006adba",
        Right::UndoPosting,
        "UndoPosting",
        "ОтменаПроведения",
    ),
    (
        "5d167fcc-b11f-403a-9a37-1eda64c19df1",
        Right::InteractivePosting,
        "InteractivePosting",
        "ИнтерактивноеПроведение",
    ),
    (
        "21b4742a-d335-4234-bf0f-a3074a0e31ac",
        Right::InteractivePostingRegular,
        "InteractivePostingRegular",
        "ИнтерактивноеПроведениеНеоперативное",
    ),
    (
        "4d0d77ec-8511-430d-bd77-8407f27bc8f4",
        Right::InteractiveUndoPosting,
        "InteractiveUndoPosting",
        "ИнтерактивнаяОтменаПроведения",
    ),
    (
        "b0c0cbfc-f2cc-4b80-8460-5d5d7a599d9d",
        Right::InteractiveChangeOfPosted,
        "InteractiveChangeOfPosted",
        "ИнтерактивноеИзменениеПроведенных",
    ),
    (
        "24abfe06-289a-48c5-8bb4-032c733e45c5",
        Right::TotalsControl,
        "TotalsControl",
        "УправлениеИтогами",
    ),
    (
        "c6de80da-a4f7-4ce9-bbeb-0b00ea564ec1",
        Right::Use,
        "Use",
        "Использование",
    ),
    (
        "3b869658-ebc9-49ff-9bb3-e7c59686f538",
        Right::InteractiveActivate,
        "InteractiveActivate",
        "ИнтерактивнаяАктивация",
    ),
    (
        "65b6855f-85d5-4d33-ab75-be4485326dd5",
        Right::Start,
        "Start",
        "Старт",
    ),
    (
        "84487e82-eb6c-4c51-ae16-3a6db17e886d",
        Right::InteractiveStart,
        "InteractiveStart",
        "ИнтерактивныйСтарт",
    ),
    (
        "74fd69fa-368e-4292-956a-65eb2f9877bd",
        Right::Execute,
        "Execute",
        "Выполнение",
    ),
    (
        "5e664189-f0ee-439c-bdc5-eb81cca41ddf",
        Right::InteractiveExecute,
        "InteractiveExecute",
        "ИнтерактивноеВыполнение",
    ),
    (
        "499e8968-ca89-43f0-9955-8756058b1b53",
        Right::Get,
        "Get",
        "Получение",
    ),
    (
        "1d306db2-d97e-4b57-9b28-5d21e838cd9e",
        Right::Set,
        "Set",
        "Установка",
    ),
    (
        "900e3c92-6e18-4874-846a-b28780b5b54c",
        Right::Administration,
        "Administration",
        "Администрирование",
    ),
    (
        "10b8ce49-ae3d-4a2e-afe7-1e3648bd59f7",
        Right::DataAdministration,
        "DataAdministration",
        "АдминистрированиеДанных",
    ),
    (
        "4d87a22d-ca7f-40ba-a367-a4eae62f4a7f",
        Right::UpdateDataBaseConfiguration,
        "UpdateDataBaseConfiguration",
        "ОбновлениеКонфигурацииБазыДанных",
    ),
    (
        "8fb221e3-0d4f-43f2-ad71-1984cad63375",
        Right::ExclusiveMode,
        "ExclusiveMode",
        "МонопольныйРежим",
    ),
    (
        "fd05f656-7a23-43a4-8996-f480a806fb97",
        Right::ActiveUsers,
        "ActiveUsers",
        "АктивныеПользователи",
    ),
    (
        "1c799cf9-342d-4bf7-9b6f-951a009228ce",
        Right::EventLog,
        "EventLog",
        "ЖурналРегистрации",
    ),
    (
        "3c00c6ee-844e-4620-85e4-671e72f114d9",
        Right::ThinClient,
        "ThinClient",
        "ТонкийКлиент",
    ),
    (
        "bd33c881-192c-4ef7-a51d-b146e38c5078",
        Right::WebClient,
        "WebClient",
        "ВебКлиент",
    ),
    (
        "1e50809b-73ed-4935-bb77-2616c4cabdf5",
        Right::MobileClient,
        "MobileClient",
        "МобильныйКлиент",
    ),
    (
        "29da0973-3b85-40e5-89da-bce02dbab08e",
        Right::ThickClient,
        "ThickClient",
        "ТолстыйКлиент",
    ),
    (
        "02119c69-f08a-4142-9426-3725d74b7719",
        Right::ExternalConnection,
        "ExternalConnection",
        "ВнешнееСоединение",
    ),
    (
        "07ef4641-f7da-417a-bd75-35c40a17c2f7",
        Right::Automation,
        "Automation",
        "Automation",
    ),
    (
        "265eec41-3ce1-4a07-bc3b-253d44c9a4f4",
        Right::TechnicalSpecialistMode,
        "TechnicalSpecialistMode",
        "РежимТехническогоСпециалиста",
    ),
    (
        "3762abec-3836-446a-83ce-3e05001bca8b",
        Right::CollaborationSystemInfoBaseRegistration,
        "CollaborationSystemInfoBaseRegistration",
        "РегистрацияИнформационнойБазыСистемыВзаимодействия",
    ),
    (
        "d066966a-ff6a-4a41-bd68-6191cab083bc",
        Right::MainWindowModeNormal,
        "MainWindowModeNormal",
        "РежимОсновногоОкнаОбычный",
    ),
    (
        "f6168734-8b8d-4a88-ab39-ef6b51758e83",
        Right::MainWindowModeWorkplace,
        "MainWindowModeWorkplace",
        "РежимОсновногоОкнаРабочееМесто",
    ),
    (
        "b9b44b51-3ac9-47cd-8b5a-df51afdcceb0",
        Right::MainWindowModeEmbeddedWorkplace,
        "MainWindowModeEmbeddedWorkplace",
        "РежимОсновногоОкнаВстроенноеРабочееМесто",
    ),
    (
        "818fc6c3-4691-44e3-a80c-e8d424730ead",
        Right::MainWindowModeFullscreenWorkplace,
        "MainWindowModeFullscreenWorkplace",
        "РежимОсновногоОкнаПолноэкранноеРабочееМесто",
    ),
    (
        "155a0b35-4343-4047-989b-d385373b063e",
        Right::MainWindowModeKiosk,
        "MainWindowModeKiosk",
        "РежимОсновногоОкнаКиоск",
    ),
    (
        "f7c6a0bb-bca6-4cd3-9146-832971cd7073",
        Right::AnalyticsSystemClient,
        "AnalyticsSystemClient",
        "КлиентАналитическойСистемы",
    ),
    (
        "4df6d046-3bf8-4dda-991c-53ba664296a5",
        Right::SaveUserData,
        "SaveUserData",
        "СохранениеДанныхПользователя",
    ),
    (
        "d8682bbb-7800-4aa0-8590-d3cb11fe2a29",
        Right::ConfigurationExtensionsAdministration,
        "ConfigurationExtensionsAdministration",
        "АдминистрированиеРасширенийКонфигурации",
    ),
    (
        "399d7390-8d83-4a57-b4d7-c902c15b701f",
        Right::InteractiveOpenExtDataProcessors,
        "InteractiveOpenExtDataProcessors",
        "ИнтерактивноеОткрытиеВнешнихОбработок",
    ),
    (
        "7b8359dd-7d4e-4bcd-a61c-b4b26eae19c6",
        Right::InteractiveOpenExtReports,
        "InteractiveOpenExtReports",
        "ИнтерактивноеОткрытиеВнешнихОтчетов",
    ),
    (
        "31c3d4f6-7d02-4654-a14e-06aacafcb4fa",
        Right::Output,
        "Output",
        "Вывод",
    ),
];

impl Right {
    /// Names a right by its platform identifier.
    #[must_use]
    pub fn from_guid(guid: &Guid) -> Self {
        RIGHTS
            .iter()
            .find(|(identifier, ..)| identifier.eq_ignore_ascii_case(guid.as_str()))
            .map_or_else(|| Self::Other(guid.clone()), |(_, right, ..)| right.clone())
    }

    /// The identifier of the right.
    #[must_use]
    pub fn guid(&self) -> Guid {
        match self {
            Self::Other(guid) => guid.clone(),
            named => RIGHTS
                .iter()
                .find(|(_, right, ..)| right == named)
                .and_then(|(identifier, ..)| Guid::from_str(identifier).ok())
                .expect("every named right has an identifier"),
        }
    }

    /// The platform name of the right (`Read`), or the identifier of an
    /// unnamed one.
    #[must_use]
    pub fn name(&self) -> String {
        match self {
            Self::Other(guid) => guid.as_str().to_owned(),
            named => RIGHTS
                .iter()
                .find(|(_, right, ..)| right == named)
                .map(|(_, _, name, _)| (*name).to_owned())
                .expect("every named right has a name"),
        }
    }

    /// The Russian name of the right (`Чтение`), or the identifier of an
    /// unnamed one.
    #[must_use]
    pub fn russian_name(&self) -> String {
        match self {
            Self::Other(guid) => guid.as_str().to_owned(),
            named => RIGHTS
                .iter()
                .find(|(_, right, ..)| right == named)
                .map(|(_, _, _, russian)| (*russian).to_owned())
                .expect("every named right has a Russian name"),
        }
    }

    /// Names a right by either of its spellings, case-insensitively.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        RIGHTS
            .iter()
            .find(|(_, _, english, russian)| {
                english.eq_ignore_ascii_case(name) || russian.to_lowercase() == name.to_lowercase()
            })
            .map(|(_, right, ..)| right.clone())
    }
}

/// The decoded rights resource of a role.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleRights {
    /// `setForNewObjects`: rights are set for objects added later.
    pub set_for_new_objects: bool,
    /// `setForAttributesByDefault`: attribute rights follow the object's.
    pub set_for_attributes_by_default: bool,
    /// `independentRightsOfChildObjects`.
    pub independent_rights_of_child_objects: bool,
    /// The rights per object, in resource order.
    pub objects: Vec<ObjectRights>,
    /// The restriction templates the conditions call, in resource order.
    pub templates: Vec<RestrictionTemplate>,
}

impl RoleRights {
    /// The rights the role records on one object, ignoring its members.
    #[must_use]
    pub fn object(&self, object: &Guid) -> Option<&ObjectRights> {
        self.objects
            .iter()
            .find(|entry| entry.members.is_empty() && &entry.object == object)
    }

    /// Whether the role grants a right on an object. The resource records
    /// only what differs from the role's default: a right it does not
    /// list — of a listed object or of an object it does not list at all
    /// — is granted when `setForNewObjects` holds and refused otherwise,
    /// which is how `ПолныеПрава` lists nothing but its refusals.
    #[must_use]
    pub fn grants(&self, object: &Guid, right: &Right) -> bool {
        self.object(object)
            .and_then(|entry| entry.right(right))
            .map_or(self.set_for_new_objects, |entry| entry.allowed)
    }

    /// The restrictions the role attaches to a right of an object, empty
    /// when it lists none.
    #[must_use]
    pub fn restrictions(&self, object: &Guid, right: &Right) -> &[Restriction] {
        self.object(object)
            .and_then(|entry| entry.right(right))
            .map_or(&[], |entry| entry.restrictions.as_slice())
    }

    /// The template of a name, without its parameter list.
    #[must_use]
    pub fn template(&self, name: &str) -> Option<&RestrictionTemplate> {
        self.templates
            .iter()
            .find(|template| template.name.to_lowercase() == name.to_lowercase())
    }
}

/// The rights a role records on one metadata object or on one member of
/// it (an attribute, a tabular section, a command).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectRights {
    /// The metadata object.
    pub object: Guid,
    /// The member path below the object, empty for the object itself;
    /// each step is the raw kind marker and identifier the resource writes.
    pub members: Vec<ObjectMember>,
    /// The rights listed, in resource order.
    pub rights: Vec<GrantedRight>,
}

impl ObjectRights {
    /// The entry of one right, if the role lists it.
    #[must_use]
    pub fn right(&self, right: &Right) -> Option<&GrantedRight> {
        self.rights.iter().find(|entry| &entry.right == right)
    }
}

/// One step of a member path in a rights entry, kept as written.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectMember {
    /// The kind marker the resource writes before the identifier.
    pub kind: i64,
    /// The member identifier.
    pub id: Guid,
}

/// One right of an object as a role records it.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantedRight {
    /// The right.
    pub right: Right,
    /// Whether the role grants it (`1`) or refuses it (`-1`).
    pub allowed: bool,
    /// The row-level restrictions attached to the right.
    pub restrictions: Vec<Restriction>,
}

/// One row-level restriction of a right.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restriction {
    /// The condition text in the restriction language, with preprocessor
    /// directives and template calls as written.
    pub condition: String,
    /// The fields the restriction is set for, by identifier; empty for
    /// every field.
    pub fields: Vec<Guid>,
}

/// One restriction template of a role.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestrictionTemplate {
    /// The template name, `ДляОбъекта`.
    pub name: String,
    /// The parameter names of the signature, `ПолеОбъекта`.
    pub parameters: Vec<String>,
    /// The body, with `#Параметр` references.
    pub body: String,
}

/// Decodes a role's rights resource.
///
/// # Errors
///
/// Returns [`MetadataError`] when the resource is not raw DEFLATE, not
/// brace-serialized, or not a rights resource of the known format.
pub fn parse_role_rights(compressed: &[u8]) -> Result<RoleRights, MetadataError> {
    parse_role_rights_bounded(compressed, super::deflate::DEFAULT_OUTPUT_LIMIT)
}

/// [`parse_role_rights`] with a bound on the inflated size.
///
/// # Errors
///
/// As [`parse_role_rights`], plus the size bound.
pub fn parse_role_rights_bounded(
    compressed: &[u8],
    decoded_limit: usize,
) -> Result<RoleRights, MetadataError> {
    let decoded = inflate_raw_deflate_bounded(compressed, decoded_limit)?;
    let value = parse_serialized(&decoded)?;
    rights_from_value(&value)
}

fn malformed(message: &str) -> MetadataError {
    MetadataError::new(
        MetadataErrorKind::Serialization,
        format!("rights resource: {message}"),
    )
}

fn rights_from_value(value: &Value) -> Result<RoleRights, MetadataError> {
    let top = value.as_list().ok_or_else(|| malformed("not a list"))?;
    match top.first().and_then(Value::as_u32) {
        Some(RIGHTS_FORMAT_VERSION) => {}
        Some(other) => {
            return Err(malformed(&format!(
                "format version {other} is not supported (expected {RIGHTS_FORMAT_VERSION})"
            )));
        }
        None => return Err(malformed("no format version")),
    }
    let entries = counted_list(top.get(1), "object list")?;
    let mut objects = Vec::with_capacity(entries.len());
    for entry in entries {
        objects.push(object_rights(entry)?);
    }
    let mut templates = Vec::new();
    for template in counted_list(top.get(2), "template list")? {
        templates.push(restriction_template(template)?);
    }
    let flag = |index: usize| {
        top.get(index)
            .is_some_and(|value| value.as_scalar() == Some("1"))
    };
    Ok(RoleRights {
        set_for_new_objects: flag(3),
        set_for_attributes_by_default: flag(4),
        independent_rights_of_child_objects: flag(5),
        objects,
        templates,
    })
}

/// The items of a `{N, item…}` list, checked against its count.
fn counted_list<'value>(
    value: Option<&'value Value>,
    what: &str,
) -> Result<&'value [Value], MetadataError> {
    let list = value
        .and_then(Value::as_list)
        .ok_or_else(|| malformed(&format!("{what} is not a list")))?;
    let count = list
        .first()
        .and_then(Value::as_u32)
        .ok_or_else(|| malformed(&format!("{what} has no count")))?;
    let items = &list[1..];
    if items.len() != count as usize {
        return Err(malformed(&format!(
            "{what} declares {count} items and carries {}",
            items.len()
        )));
    }
    Ok(items)
}

fn guid_at(list: &[Value], index: usize, what: &str) -> Result<Guid, MetadataError> {
    list.get(index)
        .and_then(Value::as_scalar)
        .and_then(|guid| Guid::from_str(guid).ok())
        .ok_or_else(|| malformed(&format!("{what} has no identifier")))
}

fn integer_at(list: &[Value], index: usize, what: &str) -> Result<i64, MetadataError> {
    let scalar = list
        .get(index)
        .and_then(Value::as_scalar)
        .ok_or_else(|| malformed(&format!("{what} has no number")))?;
    // Unsigned 32-bit spellings of negative numbers appear beside signed
    // ones: `4294967295` is `-1`.
    if let Ok(unsigned) = scalar.parse::<u32>() {
        return Ok(i64::from(unsigned as i32));
    }
    scalar
        .parse::<i64>()
        .map_err(|_| malformed(&format!("{what} has no number")))
}

fn object_rights(entry: &Value) -> Result<ObjectRights, MetadataError> {
    let pair = entry
        .as_list()
        .filter(|pair| pair.len() == 2)
        .ok_or_else(|| malformed("object entry is not a header and a rights list"))?;
    let header = pair[0]
        .as_list()
        .ok_or_else(|| malformed("object header is not a list"))?;
    let object = guid_at(header, 1, "object header")?;
    let member_count = integer_at(header, 2, "object header")?;
    let mut members = Vec::new();
    for index in 0..usize::try_from(member_count).unwrap_or(0) {
        let member = header
            .get(3 + index)
            .and_then(Value::as_list)
            .ok_or_else(|| malformed("object member is not a list"))?;
        members.push(ObjectMember {
            kind: integer_at(member, 0, "object member")?,
            id: guid_at(member, 1, "object member")?,
        });
    }
    let list = pair[1]
        .as_list()
        .ok_or_else(|| malformed("rights list is not a list"))?;
    let restricted = integer_at(list, 0, "rights list")? == 1;
    let (pairs, nodes): (&[Value], &[Value]) = if restricted {
        let count = usize::try_from(integer_at(list, 1, "rights list")?).unwrap_or(0);
        let end = 2 + 2 * count;
        let pairs = list
            .get(2..end)
            .ok_or_else(|| malformed("rights list is shorter than its count"))?;
        // The restriction nodes follow their count inline.
        let node_count = usize::try_from(integer_at(list, end, "restriction count")?).unwrap_or(0);
        let nodes = list
            .get(end + 1..end + 1 + node_count)
            .ok_or_else(|| malformed("restriction list is shorter than its count"))?;
        (pairs, nodes)
    } else {
        (&list[1..], &[])
    };
    let mut rights = Vec::with_capacity(pairs.len() / 2);
    for pair in pairs.chunks(2) {
        let right = Right::from_guid(&guid_at(pair, 0, "right")?);
        let allowed = integer_at(pair, 1, "right").is_ok_and(|value| value == 1);
        rights.push(GrantedRight {
            right,
            allowed,
            restrictions: Vec::new(),
        });
    }
    for node in nodes {
        let node = node
            .as_list()
            .ok_or_else(|| malformed("restriction node is not a list"))?;
        let right = Right::from_guid(&guid_at(node, 0, "restriction node")?);
        let mut restrictions = Vec::new();
        for condition in counted_list(node.get(1), "restriction conditions")? {
            restrictions.push(restriction(condition)?);
        }
        match rights.iter_mut().find(|entry| entry.right == right) {
            Some(entry) => entry.restrictions.extend(restrictions),
            None => rights.push(GrantedRight {
                right,
                allowed: true,
                restrictions,
            }),
        }
    }
    Ok(ObjectRights {
        object,
        members,
        rights,
    })
}

fn restriction(value: &Value) -> Result<Restriction, MetadataError> {
    let list = value
        .as_list()
        .ok_or_else(|| malformed("restriction is not a list"))?;
    let condition = list
        .get(1)
        .and_then(Value::as_string)
        .ok_or_else(|| malformed("restriction has no condition text"))?
        .to_owned();
    let mut fields = Vec::new();
    for field in list.get(3).and_then(Value::as_list).unwrap_or(&[]) {
        // `{0}` marks the object itself, `{0, <guid>}` one field.
        if let Some(guid) = field
            .as_list()
            .and_then(|field| field.get(1))
            .and_then(Value::as_scalar)
            .and_then(|guid| Guid::from_str(guid).ok())
        {
            fields.push(guid);
        }
    }
    Ok(Restriction { condition, fields })
}

fn restriction_template(value: &Value) -> Result<RestrictionTemplate, MetadataError> {
    let list = value
        .as_list()
        .ok_or_else(|| malformed("template is not a list"))?;
    let signature = list
        .first()
        .and_then(Value::as_string)
        .ok_or_else(|| malformed("template has no name"))?;
    let body = list
        .get(1)
        .and_then(Value::as_string)
        .ok_or_else(|| malformed("template has no body"))?
        .to_owned();
    let (name, parameters) = match signature.split_once('(') {
        Some((name, rest)) => (
            name.trim().to_owned(),
            rest.trim_end_matches(')')
                .split(',')
                .map(str::trim)
                .filter(|parameter| !parameter.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
        ),
        None => (signature.trim().to_owned(), Vec::new()),
    };
    Ok(RestrictionTemplate {
        name,
        parameters,
        body,
    })
}

/// The role identifiers the roles collection of a decoded resource
/// lists, or none when the resource carries no such collection.
#[must_use]
pub fn roles_collection(decoded: &[u8]) -> Vec<Guid> {
    let marker = ROLES_COLLECTION.as_bytes();
    let Some(start) = decoded
        .windows(marker.len())
        .position(|window| window.eq_ignore_ascii_case(marker))
    else {
        return Vec::new();
    };
    // `{<collection>, N, <guid>, …}`: the identifiers follow the count.
    let mut rest = &decoded[start + marker.len()..];
    let mut roles = Vec::new();
    let mut count_seen = false;
    loop {
        rest = skip_separators(rest);
        if rest.first() == Some(&b'}') || rest.is_empty() {
            break;
        }
        let end = rest
            .iter()
            .position(|byte| matches!(byte, b',' | b'}' | b'{'))
            .unwrap_or(rest.len());
        let token = std::str::from_utf8(&rest[..end]).unwrap_or("").trim();
        if !count_seen {
            count_seen = true;
        } else if let Ok(guid) = Guid::from_str(token) {
            roles.push(guid);
        } else {
            break;
        }
        rest = &rest[end..];
    }
    roles
}

fn skip_separators(bytes: &[u8]) -> &[u8] {
    let count = bytes
        .iter()
        .take_while(|byte| matches!(byte, b',' | b' ' | b'\r' | b'\n' | b'\t'))
        .count();
    &bytes[count..]
}

/// One role of a configuration.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleEntry {
    /// The role identifier, the bare-GUID resource name.
    pub guid: Guid,
    /// The metadata name.
    pub name: String,
    /// The first synonym, when the descriptor has one.
    pub synonym: Option<String>,
}

/// The roles a configuration declares, named from their descriptors.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RoleCatalog {
    roles: Vec<RoleEntry>,
}

impl RoleCatalog {
    /// Names the roles of the collection from the descriptors; a role
    /// without a descriptor is listed under its identifier.
    #[must_use]
    pub fn from_descriptors(roles: &[Guid], descriptors: &[ConfigDescriptor]) -> Self {
        let roles = roles
            .iter()
            .map(|guid| {
                let descriptor = descriptors.iter().find(|descriptor| {
                    &descriptor.object_guid == guid && &descriptor.resource_guid == guid
                });
                RoleEntry {
                    guid: guid.clone(),
                    name: descriptor.map_or_else(
                        || guid.as_str().to_owned(),
                        |descriptor| descriptor.name.clone(),
                    ),
                    synonym: descriptor
                        .and_then(|descriptor| descriptor.synonyms.first())
                        .map(|synonym| synonym.text.clone()),
                }
            })
            .collect();
        Self { roles }
    }

    /// Names the roles a snapshot carries from its descriptors.
    #[must_use]
    pub fn from_snapshot(snapshot: &super::MetadataSnapshot) -> Self {
        Self::from_descriptors(snapshot.roles(), snapshot.descriptors())
    }

    /// Every role, in collection order.
    #[must_use]
    pub fn roles(&self) -> &[RoleEntry] {
        &self.roles
    }

    /// The role of a name, case-insensitively.
    #[must_use]
    pub fn by_name(&self, name: &str) -> Option<&RoleEntry> {
        self.roles
            .iter()
            .find(|role| role.name.to_lowercase() == name.to_lowercase())
    }

    /// The role of an identifier.
    #[must_use]
    pub fn by_guid(&self, guid: &Guid) -> Option<&RoleEntry> {
        self.roles.iter().find(|role| &role.guid == guid)
    }
}
