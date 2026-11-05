//! The complete UserData authentication path, not the direct Stored override.
use super::protocol::{Raw, codes, error_of};
use super::*;
use open_sdbl::metadata::decode_user_data;
use open_sdbl::password::{VerificationOptions, VerificationStatus};

#[path = "../../../../tests/support/password_record.rs"]
mod record;

#[tokio::test]
async fn native_shaped_extended_records_authenticate_through_user_data() {
    for tag in ["sha256", "sha512"] {
        let vector = include_str!("../../../../tests/fixtures/password/vectors.tsv")
            .lines()
            .find(|l| l.starts_with(&format!("{tag}-match-public-stored-value-8.3.27\t")))
            .unwrap();
        let stored = vector
            .split('\t')
            .nth(5)
            .unwrap()
            .split(',')
            .collect::<Vec<_>>();
        for optional in [2, 5] {
            let blob = record::blob(stored[0], stored[1], stored[2], stored[3], optional);
            let snapshot = fixture::snapshot();
            let access = fixture::store_with(&snapshot, true, |user| {
                if user.name == "Администратор" {
                    user.data = decode_user_data(&blob).unwrap();
                    user.standard_authentication = true;
                }
            });
            let harness = Harness::new(Options {
                access: Some(access),
                ..Options::default()
            });
            let mut raw = Raw {
                stream: harness.connect(),
            };
            let frames = raw.login("Администратор", PASSWORD).await;
            assert_eq!(
                frames.last().map(|f| f.0),
                Some(b'Z'),
                "{tag} optional={optional}: {}",
                codes(&frames)
            );
            for wrong in ["wrong", "PASS123!"] {
                let mut raw = Raw {
                    stream: harness.connect(),
                };
                let frames = raw.login("Администратор", wrong).await;
                assert_eq!(error_of(&frames)[&b'C'], "28P01");
            }
            harness.stop();
        }
    }
}

#[tokio::test]
#[ignore = "read-only live v8users check; requires OPEN_SDBL_AUTH_IB_PASSWORD and local psql/pgpass"]
async fn live_user_record_authenticates_without_exposing_credentials() {
    let candidate = Zeroizing::new(
        std::env::var("OPEN_SDBL_AUTH_IB_PASSWORD")
            .expect("OPEN_SDBL_AUTH_IB_PASSWORD is required"),
    );
    let user = std::env::var("OPEN_SDBL_AUTH_IB_USER").unwrap_or_else(|_| "Администратор".into());
    let host = std::env::var("OPEN_SDBL_AUTH_DB_HOST").unwrap_or_else(|_| "localhost".into());
    let database = std::env::var("OPEN_SDBL_AUTH_DB_NAME").unwrap_or_else(|_| "unf".into());
    let database_user =
        std::env::var("OPEN_SDBL_AUTH_DB_USER").unwrap_or_else(|_| "admin1c".into());
    let sql = format!(
        "select encode(data,'hex'),eauth from v8users where name='{}'",
        user.replace('\'', "''")
    );
    let output = std::process::Command::new("psql")
        .args([
            "-X",
            "-w",
            "-At",
            "-h",
            &host,
            "-U",
            &database_user,
            "-d",
            &database,
            "-c",
            &sql,
        ])
        .env("PGCONNECT_TIMEOUT", "5")
        .output()
        .expect("psql is installed");
    assert!(output.status.success(), "read-only user query failed");
    let row = Zeroizing::new(String::from_utf8(output.stdout).expect("psql returns UTF-8"));
    let fields = row.trim().split('|').collect::<Vec<_>>();
    assert_eq!(fields.len(), 2, "exactly one user row is required");
    assert_eq!(fields[1], "t", "standard authentication must be enabled");
    let bytes = Zeroizing::new(
        (0..fields[0].len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&fields[0][i..i + 2], 16).unwrap())
            .collect::<Vec<_>>(),
    );
    let data = decode_user_data(&bytes).expect("user record decodes");
    let verification = data.verify_password(&candidate, &VerificationOptions::new());
    assert_eq!(
        verification.status(),
        VerificationStatus::Match,
        "reason={:?}, profile={:?}",
        verification.reason(),
        verification.profile()
    );
    eprintln!(
        "live record: primary MATCH, profile={:?}",
        verification.profile()
    );
    let snapshot = fixture::snapshot();
    let mut access = fixture::store(&snapshot, true);
    let mut users = access.users().unwrap().to_vec();
    let selected = users
        .iter_mut()
        .find(|u| u.name == "Администратор")
        .unwrap();
    selected.name = user.clone();
    selected.standard_authentication = true;
    selected.data = data;
    access.set_users(users);
    let harness = Harness::new(Options {
        access: Some(access),
        ..Options::default()
    });
    let mut raw = Raw {
        stream: harness.connect(),
    };
    let frames = raw.login(&user, &candidate).await;
    assert_eq!(
        frames.last().map(|f| f.0),
        Some(b'Z'),
        "live record failed gateway authentication"
    );
    let mut raw = Raw {
        stream: harness.connect(),
    };
    assert_eq!(
        error_of(&raw.login(&user, "definitely-wrong-password").await)[&b'C'],
        "28P01"
    );
    harness.stop();
}
