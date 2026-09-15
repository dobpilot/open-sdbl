use std::io;
use std::net::IpAddr;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use zeroize::Zeroizing;

use crate::CONNECTION_TIMEOUT;
use crate::auth::pgpass::EnvironmentSecret;
use crate::error::CliError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Socks5Proxy {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) username: Option<String>,
}

pub(crate) fn parse_socks5_proxy(value: &str) -> Result<Socks5Proxy, &'static str> {
    let (host, port) = if let Some(bracketed) = value.strip_prefix('[') {
        let (host, port) = bracketed.split_once("]:").ok_or("expected [IPv6]:PORT")?;
        match host.parse::<IpAddr>() {
            Ok(IpAddr::V6(_)) => (host, port),
            _ => return Err("brackets are only valid around an IPv6 address"),
        }
    } else {
        let (host, port) = value.rsplit_once(':').ok_or("expected HOST:PORT")?;
        if host.contains(':') {
            return Err("IPv6 addresses must be enclosed in brackets");
        }
        (host, port)
    };

    if host.is_empty() || host.trim() != host {
        return Err("host must not be empty or contain surrounding whitespace");
    }
    let port = port
        .parse::<u16>()
        .map_err(|_| "port must be an integer from 1 to 65535")?;
    if port == 0 {
        return Err("port must be an integer from 1 to 65535");
    }
    Ok(Socks5Proxy {
        host: host.to_owned(),
        port,
        username: None,
    })
}

pub(crate) fn socks5_password(
    proxy: Option<&Socks5Proxy>,
    environment: &EnvironmentSecret,
) -> Result<Option<Zeroizing<String>>, CliError> {
    let Some(username) = proxy.and_then(|proxy| proxy.username.as_ref()) else {
        return Ok(None);
    };
    if username.is_empty() || username.len() > usize::from(u8::MAX) {
        return Err(CliError::Data(
            "SOCKS5 username must contain from 1 to 255 bytes".to_owned(),
        ));
    }
    let password = environment.required("SOCKS5_PASSWORD")?;
    if password.is_empty() || password.len() > usize::from(u8::MAX) {
        return Err(CliError::Data(
            "SOCKS5_PASSWORD must contain from 1 to 255 bytes".to_owned(),
        ));
    }
    Ok(Some(password))
}

pub(crate) async fn connect_socks5(
    proxy: &Socks5Proxy,
    password: Option<&str>,
    target_host: &str,
    target_port: u16,
) -> Result<TcpStream, CliError> {
    let request =
        socks5_connect_request(target_host, target_port).map_err(CliError::socks5_connection)?;
    let negotiation = async {
        let mut stream = TcpStream::connect((proxy.host.as_str(), proxy.port)).await?;

        if proxy.username.is_some() {
            // Credentials are an explicit policy choice. Do not advertise the
            // unauthenticated method, which would allow a proxy to downgrade
            // the connection silently.
            stream.write_all(&[0x05, 0x01, 0x02]).await?;
        } else {
            stream.write_all(&[0x05, 0x01, 0x00]).await?;
        }
        let mut method = [0_u8; 2];
        stream.read_exact(&mut method).await?;
        match method {
            [0x05, 0x00] if proxy.username.is_none() => {}
            [0x05, 0x00] => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "proxy selected unauthenticated SOCKS5 access despite configured credentials",
                ));
            }
            [0x05, 0x02] => {
                let username = proxy.username.as_deref().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "proxy requires SOCKS5 username/password authentication",
                    )
                })?;
                let password = password.ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "SOCKS5 password is unavailable",
                    )
                })?;
                let username_length = u8::try_from(username.len()).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidInput, "SOCKS5 username is too long")
                })?;
                let password_length = u8::try_from(password.len()).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidInput, "SOCKS5 password is too long")
                })?;
                let mut authentication =
                    Zeroizing::new(Vec::with_capacity(username.len() + password.len() + 3));
                authentication.extend_from_slice(&[0x01, username_length]);
                authentication.extend_from_slice(username.as_bytes());
                authentication.push(password_length);
                authentication.extend_from_slice(password.as_bytes());
                stream.write_all(authentication.as_slice()).await?;
                let mut response = [0_u8; 2];
                stream.read_exact(&mut response).await?;
                match response {
                    [0x01, 0x00] => {}
                    [0x01, _] => {
                        return Err(io::Error::new(
                            io::ErrorKind::PermissionDenied,
                            "proxy rejected SOCKS5 username/password authentication",
                        ));
                    }
                    [version, _] => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!(
                                "proxy returned unexpected SOCKS5 authentication version 0x{version:02x}"
                            ),
                        ));
                    }
                }
            }
            [0x05, 0xff] => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "proxy rejected unauthenticated SOCKS5 access",
                ));
            }
            [0x05, selected] => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    format!("proxy selected unsupported authentication method 0x{selected:02x}"),
                ));
            }
            [version, _] => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("proxy returned unexpected SOCKS version 0x{version:02x}"),
                ));
            }
        }

        stream.write_all(&request).await?;
        let mut response = [0_u8; 4];
        stream.read_exact(&mut response).await?;
        if response[0] != 0x05 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "proxy returned unexpected SOCKS version 0x{:02x}",
                    response[0]
                ),
            ));
        }
        if response[1] != 0x00 {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                socks5_reply_message(response[1]),
            ));
        }
        if response[2] != 0x00 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "proxy returned a malformed SOCKS5 response",
            ));
        }

        let bound_address_len = match response[3] {
            0x01 => 4,
            0x03 => {
                let mut length = [0_u8; 1];
                stream.read_exact(&mut length).await?;
                usize::from(length[0])
            }
            0x04 => 16,
            address_type => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("proxy returned unknown address type 0x{address_type:02x}"),
                ));
            }
        };
        let mut bound_address_and_port = vec![0_u8; bound_address_len + 2];
        stream.read_exact(&mut bound_address_and_port).await?;
        Ok(stream)
    };

    match timeout(CONNECTION_TIMEOUT, negotiation).await {
        Ok(Ok(stream)) => Ok(stream),
        Ok(Err(error)) => Err(CliError::socks5_connection(error)),
        Err(_) => Err(CliError::socks5_connection(format!(
            "timed out after {} seconds",
            CONNECTION_TIMEOUT.as_secs()
        ))),
    }
}

pub(crate) fn socks5_connect_request(target_host: &str, target_port: u16) -> io::Result<Vec<u8>> {
    let mut request = Vec::with_capacity(target_host.len() + 8);
    request.extend_from_slice(&[0x05, 0x01, 0x00]);
    match target_host.parse::<IpAddr>() {
        Ok(IpAddr::V4(address)) => {
            request.push(0x01);
            request.extend_from_slice(&address.octets());
        }
        Ok(IpAddr::V6(address)) => {
            request.push(0x04);
            request.extend_from_slice(&address.octets());
        }
        Err(_) => {
            let length = u8::try_from(target_host.len()).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "PostgreSQL hostname is too long for SOCKS5",
                )
            })?;
            request.extend_from_slice(&[0x03, length]);
            request.extend_from_slice(target_host.as_bytes());
        }
    }
    request.extend_from_slice(&target_port.to_be_bytes());
    Ok(request)
}

fn socks5_reply_message(reply: u8) -> String {
    let reason = match reply {
        0x01 => "general proxy failure",
        0x02 => "connection not allowed by proxy rules",
        0x03 => "network unreachable",
        0x04 => "host unreachable",
        0x05 => "connection refused",
        0x06 => "TTL expired",
        0x07 => "command not supported",
        0x08 => "address type not supported",
        _ => "unknown proxy error",
    };
    format!("proxy rejected CONNECT request: {reason} (0x{reply:02x})")
}

#[cfg(test)]
mod tests {
    use super::{Socks5Proxy, connect_socks5, parse_socks5_proxy, socks5_connect_request};
    use crate::db::postgres::connect_postgres_raw;

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
        let stream = connect_socks5(&proxy, None, "database.internal", 15432)
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

        let error = connect_socks5(&proxy, None, "database.internal", 5432)
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

        let stream = connect_socks5(&proxy, Some("secret"), "database.internal", 5432)
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

        let error = connect_socks5(&proxy, Some("secret"), "database.internal", 5432)
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

        let error = connect_socks5(&proxy, None, "database.internal", 5432)
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

        let error =
            match connect_postgres_raw(&configuration, stream, Duration::from_millis(25)).await {
                Ok(_) => panic!("silent PostgreSQL startup unexpectedly succeeded"),
                Err(error) => error,
            };
        assert_eq!(
            error.to_string(),
            "PostgreSQL startup through SOCKS5 timed out after 25ms"
        );
        server.await.unwrap();
    }
}
