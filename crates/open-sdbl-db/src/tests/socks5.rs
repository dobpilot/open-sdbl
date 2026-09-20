//! Tests of the `socks5` module.

use super::{Socks5Proxy, connect_socks5, parse_socks5_proxy, socks5_connect_request};
use crate::db::postgres::connect_postgres_raw;
use crate::limits::Limits;

#[test]
fn parses_socks5_proxy_endpoints() {
    assert_eq!(
        parse_socks5_proxy("proxy.example:1080").unwrap(),
        Socks5Proxy {
            host: "proxy.example".to_owned(),
            port: 1080,
            username: None,
        }
    );
    assert_eq!(
        parse_socks5_proxy("[2001:db8::1]:9050").unwrap(),
        Socks5Proxy {
            host: "2001:db8::1".to_owned(),
            port: 9050,
            username: None,
        }
    );
    assert!(parse_socks5_proxy("proxy.example").is_err());
    assert!(parse_socks5_proxy("2001:db8::1:1080").is_err());
    assert!(parse_socks5_proxy(":1080").is_err());
    assert!(parse_socks5_proxy("proxy.example:0").is_err());
}

#[test]
fn encodes_ip_targets_in_socks5_connect_requests() {
    assert_eq!(
        socks5_connect_request("192.0.2.1", 5432).unwrap(),
        vec![0x05, 0x01, 0x00, 0x01, 192, 0, 2, 1, 0x15, 0x38]
    );
    assert_eq!(
        socks5_connect_request("2001:db8::1", 15432).unwrap(),
        vec![
            0x05, 0x01, 0x00, 0x04, 0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1,
            0x3c, 0x48,
        ]
    );
}

#[tokio::test]
async fn sends_database_hostname_to_socks5_proxy() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut greeting = [0_u8; 3];
        stream.read_exact(&mut greeting).await.unwrap();
        assert_eq!(greeting, [0x05, 0x01, 0x00]);
        stream.write_all(&[0x05, 0x00]).await.unwrap();

        let mut request = [0_u8; 5];
        stream.read_exact(&mut request).await.unwrap();
        assert_eq!(&request[..4], &[0x05, 0x01, 0x00, 0x03]);
        let mut host_and_port = vec![0_u8; usize::from(request[4]) + 2];
        stream.read_exact(&mut host_and_port).await.unwrap();
        assert_eq!(
            &host_and_port[..host_and_port.len() - 2],
            b"database.internal"
        );
        assert_eq!(
            &host_and_port[host_and_port.len() - 2..],
            &15432_u16.to_be_bytes()
        );
        stream
            .write_all(&[0x05, 0x00, 0x00, 0x01, 127, 0, 0, 1, 0x12, 0x34])
            .await
            .unwrap();
    });

    let proxy = Socks5Proxy {
        host: address.ip().to_string(),
        port: address.port(),
        username: None,
    };
    let stream = connect_socks5(
        &proxy,
        None,
        "database.internal",
        15432,
        Limits::default().connection_timeout,
    )
    .await
    .unwrap();
    drop(stream);
    server.await.unwrap();
}

#[tokio::test]
async fn reports_missing_socks5_authentication_credentials() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut greeting = [0_u8; 3];
        stream.read_exact(&mut greeting).await.unwrap();
        stream.write_all(&[0x05, 0x02]).await.unwrap();
    });
    let proxy = Socks5Proxy {
        host: address.ip().to_string(),
        port: address.port(),
        username: None,
    };

    let error = connect_socks5(
        &proxy,
        None,
        "database.internal",
        5432,
        Limits::default().connection_timeout,
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(error.contains("requires SOCKS5 username/password authentication"));
    server.await.unwrap();
}

#[tokio::test]
async fn authenticates_to_socks5_with_username_and_password() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut greeting = [0_u8; 3];
        stream.read_exact(&mut greeting).await.unwrap();
        assert_eq!(greeting, [0x05, 0x01, 0x02]);
        stream.write_all(&[0x05, 0x02]).await.unwrap();

        let mut authentication = [0_u8; 13];
        stream.read_exact(&mut authentication).await.unwrap();
        assert_eq!(&authentication, b"\x01\x04user\x06secret");
        stream.write_all(&[0x01, 0x00]).await.unwrap();

        let mut request = [0_u8; 5];
        stream.read_exact(&mut request).await.unwrap();
        assert_eq!(&request[..4], &[0x05, 0x01, 0x00, 0x03]);
        let mut host_and_port = vec![0_u8; usize::from(request[4]) + 2];
        stream.read_exact(&mut host_and_port).await.unwrap();
        assert_eq!(
            &host_and_port[..host_and_port.len() - 2],
            b"database.internal"
        );
        stream
            .write_all(&[0x05, 0x00, 0x00, 0x01, 127, 0, 0, 1, 0x12, 0x34])
            .await
            .unwrap();
    });
    let proxy = Socks5Proxy {
        host: address.ip().to_string(),
        port: address.port(),
        username: Some("user".to_owned()),
    };

    let stream = connect_socks5(
        &proxy,
        Some("secret"),
        "database.internal",
        5432,
        Limits::default().connection_timeout,
    )
    .await
    .unwrap();
    drop(stream);
    server.await.unwrap();
}

#[tokio::test]
async fn rejects_socks5_authentication_downgrade() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut greeting = [0_u8; 3];
        stream.read_exact(&mut greeting).await.unwrap();
        assert_eq!(greeting, [0x05, 0x01, 0x02]);
        stream.write_all(&[0x05, 0x00]).await.unwrap();
    });
    let proxy = Socks5Proxy {
        host: address.ip().to_string(),
        port: address.port(),
        username: Some("user".to_owned()),
    };

    let error = connect_socks5(
        &proxy,
        Some("secret"),
        "database.internal",
        5432,
        Limits::default().connection_timeout,
    )
    .await
    .unwrap_err();
    assert!(error.to_string().contains("despite configured credentials"));
    server.await.unwrap();
}

#[tokio::test]
async fn reports_socks5_reply_before_a_malformed_reserved_byte() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut greeting = [0_u8; 3];
        stream.read_exact(&mut greeting).await.unwrap();
        stream.write_all(&[0x05, 0x00]).await.unwrap();
        let mut request = [0_u8; 5];
        stream.read_exact(&mut request).await.unwrap();
        let mut host_and_port = vec![0_u8; usize::from(request[4]) + 2];
        stream.read_exact(&mut host_and_port).await.unwrap();
        stream.write_all(&[0x05, 0x05, 0x01, 0x01]).await.unwrap();
    });
    let proxy = Socks5Proxy {
        host: address.ip().to_string(),
        port: address.port(),
        username: None,
    };

    let error = connect_socks5(
        &proxy,
        None,
        "database.internal",
        5432,
        Limits::default().connection_timeout,
    )
    .await
    .unwrap_err()
    .to_string();
    assert!(error.contains("connection refused"), "{error}");
    assert!(!error.contains("malformed"), "{error}");
    server.await.unwrap();
}

#[tokio::test]
async fn times_out_silent_postgres_startup_through_socks5() {
    use std::time::Duration;

    use tokio::io::AsyncReadExt;
    use tokio::net::{TcpListener, TcpStream};

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut startup = [0_u8; 1024];
        assert!(stream.read(&mut startup).await.unwrap() > 0);
        assert_eq!(stream.read(&mut startup).await.unwrap(), 0);
    });
    let stream = TcpStream::connect(address).await.unwrap();
    let mut configuration = tokio_postgres::Config::new();
    configuration.user("reader").dbname("test");

    let error = match connect_postgres_raw(&configuration, stream, Duration::from_millis(25)).await
    {
        Ok(_) => panic!("silent PostgreSQL startup unexpectedly succeeded"),
        Err(error) => error,
    };
    assert_eq!(
        error.to_string(),
        "PostgreSQL startup through SOCKS5 timed out after 25ms"
    );
    server.await.unwrap();
}
