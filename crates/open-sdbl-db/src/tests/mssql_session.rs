//! Tests of the `mssql session` module.

use super::apply_mssql_cleanup;
use super::*;
use crate::error::DbError;
use std::time::Duration;

#[test]
fn timeout_requires_dropping_mssql_without_rollback() {
    let timeout = DbError::DatabaseTimeout {
        operation: "MSSQL user query".to_owned(),
        duration: Duration::from_secs(120),
    };
    assert!(should_disconnect_after_mssql_error(&timeout));

    let semantic_error = DbError::Data("invalid row".to_owned());
    assert!(!should_disconnect_after_mssql_error(&semantic_error));
}

#[test]
fn failed_mssql_cleanup_poisons_the_session_state() {
    let mut poisoned = false;
    let error =
        apply_mssql_cleanup(&mut poisoned, Err("connection lost".to_owned()), Ok(0)).unwrap_err();
    assert!(poisoned);
    assert!(error.contains("ROLLBACK failed"));

    let mut poisoned = false;
    let error = apply_mssql_cleanup(&mut poisoned, Ok(()), Ok(1)).unwrap_err();
    assert!(poisoned);
    assert!(error.contains("@@TRANCOUNT remained 1"));

    let mut poisoned = false;
    apply_mssql_cleanup(&mut poisoned, Ok(()), Ok(0)).unwrap();
    assert!(!poisoned);
    // The session verifies transaction state, not role membership:
    // what rights the login holds is the operator's decision.
    assert!(super::MSSQL_TRANSACTION_COUNT.contains("@@TRANCOUNT"));
}

// Minimal scripted TDS peer. It exercises the actual session and driver futures
// without requiring a SQL Server installation or timing-dependent cancellation.
async fn read_tds_packet(peer: &mut TcpStream) -> Vec<u8> {
    use tokio::io::AsyncReadExt;
    let mut header = [0; 8];
    peer.read_exact(&mut header).await.unwrap();
    assert_eq!(header[1] & 1, 1, "test requests fit in one packet");
    let length = usize::from(u16::from_be_bytes([header[2], header[3]]));
    let mut body = vec![0; length - header.len()];
    peer.read_exact(&mut body).await.unwrap();
    body
}

async fn send_tds_packet(peer: &mut TcpStream, body: &[u8]) {
    use tokio::io::AsyncWriteExt;
    let length = u16::try_from(body.len() + 8).unwrap().to_be_bytes();
    peer.write_all(&[4, 1, length[0], length[1], 0, 0, 1, 0])
        .await
        .unwrap();
    peer.write_all(body).await.unwrap();
}

const TDS_DONE: &[u8] = &[0xfd, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

async fn scripted_session() -> (MsSqlSession, TcpStream) {
    use crate::connection::ConnectionOptions;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let handshake = async {
        let (mut peer, _) = listener.accept().await.unwrap();
        read_tds_packet(&mut peer).await;
        // PRELOGIN: encryption unsupported (all traffic stays on loopback).
        send_tds_packet(&mut peer, &[1, 0, 6, 0, 1, 0xff, 2]).await;
        read_tds_packet(&mut peer).await;
        // LOGINACK for TDS 7.4, empty program name, followed by DONE.
        let mut ack = vec![0xad, 10, 0, 1, 0x74, 0, 0, 4, 0, 0, 0, 0, 0];
        ack.extend_from_slice(TDS_DONE);
        send_tds_packet(&mut peer, &ack).await;
        peer
    };
    let connect = async {
        let mut config = MsSqlConfig::new();
        config.encryption(tiberius::EncryptionLevel::NotSupported);
        config.authentication(AuthMethod::sql_server("test", "test"));
        let socket = TcpStream::connect(address).await.unwrap();
        MsSqlClient::connect(config, socket.compat_write())
            .await
            .unwrap()
    };
    let (client, peer) = tokio::join!(connect, handshake);
    let session = MsSqlSession {
        client: Some(client),
        connection: MsSqlConnection {
            options: ConnectionOptions {
                host: address.ip().to_string(),
                port: address.port(),
                database: "test".into(),
                user: "test".into(),
                socks5_proxy: None,
            },
            trust_server_certificate: false,
            trust_ca_file: None,
            dialect_level: None,
        },
        database: "test".into(),
        backend: MsSqlBackend::new(0).unwrap(),
        product_version: "16.0".into(),
        poisoned: false,
        secrets: MsSqlSecrets {
            password: Zeroizing::new("test".into()),
            socks5_password: None,
        },
        layout: Some(StorageLayout::from_flags([1; 6])),
        limits: Limits::default(),
        explicit_transaction: false,
        transaction_operation_pending: false,
    };
    (session, peer)
}

fn tds_batch_sql(body: &[u8]) -> String {
    let headers = u32::from_le_bytes(body[..4].try_into().unwrap()) as usize;
    let words = body[headers..]
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    String::from_utf16(&words).unwrap()
}

#[tokio::test]
async fn cancelled_metadata_acquisition_disconnects_during_begin_read_and_commit() {
    use tokio::io::AsyncReadExt;
    timeout(Duration::from_secs(10), async {
        for stop_at in 0..3 {
            let (mut session, mut peer) = scripted_session().await;
            let mut acquisition = Box::pin(session.configuration_resource_names());
            let stopped = async {
                for index in 0..=stop_at {
                    let sql = tds_batch_sql(&read_tds_packet(&mut peer).await);
                    match index {
                        0 => assert_eq!(sql, "BEGIN TRANSACTION"),
                        1 => assert!(sql.contains("FileName"), "{sql}"),
                        2 => assert_eq!(sql, "COMMIT TRANSACTION"),
                        _ => unreachable!(),
                    }
                    if index < stop_at {
                        send_tds_packet(&mut peer, TDS_DONE).await;
                    }
                }
            };
            tokio::select! {
                result = &mut acquisition => panic!("acquisition completed before cancellation: {result:?}"),
                () = stopped => {}
            }
            drop(acquisition);
            assert!(session.is_dead(), "cancelled at stage {stop_at}");
            assert!(session.client.is_none());
            assert!(session.configuration_resource_names().await.is_err());
            let mut byte = [0];
            assert_eq!(peer.read(&mut byte).await.unwrap(), 0, "connection stayed open");
        }
    }).await.unwrap();
}

#[tokio::test]
async fn completed_metadata_acquisition_keeps_the_connection_reusable() {
    timeout(Duration::from_secs(10), async {
        let (mut session, mut peer) = scripted_session().await;
        let requests = async {
            for _ in 0..2 {
                assert!(
                    session
                        .configuration_resource_names()
                        .await
                        .unwrap()
                        .is_empty()
                );
                assert!(!session.is_dead());
                assert!(session.client.is_some());
            }
        };
        let responses = async {
            for _ in 0..6 {
                read_tds_packet(&mut peer).await;
                send_tds_packet(&mut peer, TDS_DONE).await;
            }
        };
        tokio::join!(requests, responses);
    })
    .await
    .unwrap();
}

fn tds_integer_row(value: i32) -> Vec<u8> {
    // COLMETADATA: one unnamed, fixed-width INT4 column; then ROW and DONE.
    let mut body = vec![0x81, 1, 0, 0, 0, 0, 0, 0, 0, 0x38, 0, 0xd1];
    body.extend_from_slice(&value.to_le_bytes());
    body.extend_from_slice(TDS_DONE);
    body
}

#[tokio::test]
async fn metadata_acquisition_guard_preserves_verified_rollback_and_closes_failed_cleanup() {
    timeout(Duration::from_secs(10), async {
        for transaction_count in [0, 1] {
            let (mut session, mut peer) = scripted_session().await;
            let cleanup = async {
                let mut source = MsSqlMetadataSource::new(&mut session);
                source.begin_readonly().await.unwrap();
                let error = source
                    .rollback_readonly(DbError::Data("invalid metadata".into()))
                    .await;
                assert!(error.to_string().contains("invalid metadata"));
                drop(source);
                assert_eq!(session.is_dead(), transaction_count != 0);
                assert_eq!(session.client.is_some(), transaction_count == 0);
            };
            let responses = async {
                assert_eq!(
                    tds_batch_sql(&read_tds_packet(&mut peer).await),
                    "BEGIN TRANSACTION"
                );
                send_tds_packet(&mut peer, TDS_DONE).await;
                assert!(tds_batch_sql(&read_tds_packet(&mut peer).await).contains("ROLLBACK"));
                send_tds_packet(&mut peer, TDS_DONE).await;
                assert!(tds_batch_sql(&read_tds_packet(&mut peer).await).contains("@@TRANCOUNT"));
                send_tds_packet(&mut peer, &tds_integer_row(transaction_count)).await;
            };
            tokio::join!(cleanup, responses);
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn cancelled_metadata_rollback_disconnects_during_cleanup_and_verification() {
    timeout(Duration::from_secs(10), async {
        for stop_at in 1..=2 {
            let (mut session, mut peer) = scripted_session().await;
            let mut cleanup = Box::pin(async {
                let mut source = MsSqlMetadataSource::new(&mut session);
                source.begin_readonly().await.unwrap();
                source.rollback_readonly(DbError::Data("invalid metadata".into())).await
            });
            let stopped = async {
                for index in 0..=stop_at {
                    read_tds_packet(&mut peer).await;
                    if index < stop_at {
                        send_tds_packet(&mut peer, TDS_DONE).await;
                    }
                }
            };
            tokio::select! {
                result = &mut cleanup => panic!("cleanup completed before cancellation: {result:?}"),
                () = stopped => {}
            }
            drop(cleanup);
            assert!(session.is_dead());
            assert!(session.client.is_none());
        }
    }).await.unwrap();
}
