//! Tests of the session facade.

use std::time::Duration;

use super::bounded_database_call;
use crate::error::DbError;

#[tokio::test]
async fn times_out_a_stalled_post_handshake_server_call() {
    use tokio::io::AsyncReadExt;
    use tokio::net::{TcpListener, TcpStream};

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.unwrap();
        std::future::pending::<()>().await;
    });
    let mut stream = TcpStream::connect(address).await.unwrap();
    let error = bounded_database_call("fake database query", Duration::from_millis(20), async {
        let mut byte = [0_u8; 1];
        stream
            .read_exact(&mut byte)
            .await
            .map_err(|error| DbError::Io("fake query read".to_owned(), error))?;
        Ok(())
    })
    .await
    .unwrap_err();
    assert!(error.is_database_timeout());
    assert!(error.to_string().contains("timed out"));
    server.abort();
    let _ = server.await;
}
