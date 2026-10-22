use open_sdbl::metadata::{Guid, StorageLayout};
use open_sdbl_db::extensions::{read_extension_index, read_extension_roles};
use open_sdbl_db::{
    ConnectionOptions, Credentials, DatabaseConnection, DatabaseSession, Limits,
    PostgresConnection, PostgresSslMode,
};
#[tokio::main]
async fn main() {
    let connection = DatabaseConnection::Postgres(PostgresConnection {
        options: ConnectionOptions {
            host: "127.0.0.1".into(),
            port: 55439,
            database: "sdbl_empty_role_control".into(),
            user: "sdbl_control".into(),
            socks5_proxy: None,
        },
        sslmode: PostgresSslMode::Disable,
        trust_ca_file: None,
    });
    let mut session = DatabaseSession::connect(
        &connection,
        &Credentials::new(None, None, None),
        Limits::default(),
    )
    .await
    .unwrap();
    let role = "2bc105dd-b388-436d-9410-a756e3415d30"
        .parse::<Guid>()
        .unwrap();
    let index = read_extension_index(&mut session, StorageLayout::MODERN)
        .await
        .unwrap();
    let result =
        read_extension_roles(&mut session, StorageLayout::MODERN, &index, &[role.clone()]).await;
    let case = std::env::var("CONTROL_CASE").unwrap_or_else(|_| "empty".into());
    if case == "corrupt" {
        assert!(result.is_err(), "declared corrupt rights must fail");
        println!("declared_corrupt_rights_refused=true");
        return;
    }
    let (rights, descriptors) = result.unwrap();
    println!(
        "loaded_rights={}; named_descriptors={}",
        rights.len(),
        descriptors.len()
    );
    if case == "missing" {
        assert!(rights.is_empty());
        println!("unknown_or_unavailable_role_unreadable=true");
        return;
    }
    if case == "stored" {
        assert_eq!(rights.len(), 1);
        assert!(!rights[0].1.objects.is_empty());
        println!("stored_rights_take_precedence=true");
        return;
    }
    assert_eq!(
        rights.len(),
        1,
        "complete native role without a rights subobject must load its measured empty rights"
    );
    assert_eq!(rights[0].0, role);
    assert!(rights[0].1.objects.is_empty() && rights[0].1.templates.is_empty());
    assert!(
        !rights[0].1.set_for_new_objects
            && !rights[0].1.set_for_attributes_by_default
            && !rights[0].1.independent_rights_of_child_objects
    );
    println!("empty_role_complete=true");
}
