//! Validate the PRELOGIN response before a plaintext TDS client can send LOGIN7.
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;

const HEADER: usize = 8;
const MAX_PRELOGIN: usize = u16::MAX as usize;
pub(crate) const ENCRYPTION_REFUSED: &str =
    "MSSQL selected TLS; remove --tls-mode disable or configure a plaintext-capable endpoint";
pub(crate) const INVALID_PRELOGIN: &str = "invalid MSSQL PRELOGIN response";

pub(super) struct Transport {
    stream: TcpStream,
    prelogin: Option<Prelogin>,
}

impl Transport {
    pub(super) fn new(stream: TcpStream, plaintext: bool) -> Self {
        Self {
            stream,
            prelogin: plaintext.then(Prelogin::default),
        }
    }
}

struct Prelogin {
    wire: Vec<u8>,
    payload: Vec<u8>,
    packet_start: usize,
    filled: usize,
    replayed: usize,
    checked: bool,
}

impl Default for Prelogin {
    fn default() -> Self {
        Self {
            wire: vec![0; HEADER],
            payload: Vec::new(),
            packet_start: 0,
            filled: 0,
            replayed: 0,
            checked: false,
        }
    }
}

fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, INVALID_PRELOGIN)
}

fn verify_plaintext(payload: &[u8]) -> io::Result<()> {
    let mut encryption = None;
    let mut options = payload;
    while let Some((&token, rest)) = options.split_first() {
        if token == 0xff {
            let directory_end = payload.len() - rest.len();
            let offset = encryption
                .filter(|offset| *offset >= directory_end)
                .ok_or_else(invalid)?;
            return match payload.get(offset) {
                Some(2) => Ok(()),
                Some(0 | 1 | 3) => Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    ENCRYPTION_REFUSED,
                )),
                _ => Err(invalid()),
            };
        }
        let [hi, lo, len_hi, len_lo, rest @ ..] = rest else {
            return Err(invalid());
        };
        let start = u16::from_be_bytes([*hi, *lo]) as usize;
        let len = u16::from_be_bytes([*len_hi, *len_lo]) as usize;
        if start + len > payload.len() {
            return Err(invalid());
        }
        if token == 1 && (len != 1 || encryption.replace(start).is_some()) {
            return Err(invalid());
        }
        options = rest;
    }
    Err(invalid())
}

impl Prelogin {
    fn poll_check(&mut self, stream: &mut TcpStream, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        while !self.checked {
            while self.filled < self.wire.len() {
                let mut input = ReadBuf::new(&mut self.wire[self.filled..]);
                ready!(Pin::new(&mut *stream).poll_read(cx, &mut input))?;
                if input.filled().is_empty() {
                    return Poll::Ready(Err(invalid()));
                }
                self.filled += input.filled().len();
            }
            let header = &self.wire[self.packet_start..self.packet_start + HEADER];
            let length = u16::from_be_bytes([header[2], header[3]]) as usize;
            let end = self.packet_start + length;
            if !matches!(header[0], 4 | 18) || length <= HEADER || end > MAX_PRELOGIN {
                return Poll::Ready(Err(invalid()));
            }
            if end > self.filled {
                self.wire.resize(end, 0);
                continue;
            }
            self.payload
                .extend_from_slice(&self.wire[self.packet_start + HEADER..end]);
            if header[1] & 1 != 0 {
                verify_plaintext(&self.payload)?;
                self.checked = true;
            } else {
                if end + HEADER > MAX_PRELOGIN {
                    return Poll::Ready(Err(invalid()));
                }
                self.packet_start = end;
                self.wire.resize(end + HEADER, 0);
            }
        }
        Poll::Ready(Ok(()))
    }
}

impl AsyncRead for Transport {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        output: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if output.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        let Self { stream, prelogin } = &mut *self;
        let Some(guard) = prelogin else {
            return Pin::new(stream).poll_read(cx, output);
        };
        ready!(guard.poll_check(stream, cx))?;
        let end = guard.wire.len().min(guard.replayed + output.remaining());
        output.put_slice(&guard.wire[guard.replayed..end]);
        guard.replayed = end;
        if end == guard.wire.len() {
            *prelogin = None;
        }
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for Transport {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.stream).poll_write(cx, bytes)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_explicit_plaintext_prelogin() {
        assert!(verify_plaintext(&[1, 0, 6, 0, 1, 255, 2]).is_ok());
        assert_eq!(
            verify_plaintext(&[1, 0, 6, 0, 1, 255, 3])
                .unwrap_err()
                .to_string(),
            ENCRYPTION_REFUSED
        );
        for invalid in [
            &[255][..],
            &[1, 0, 3, 0, 1, 255, 2],
            &[1, 0, 6, 0, 1, 255, 4],
        ] {
            assert_eq!(
                verify_plaintext(invalid).unwrap_err().to_string(),
                INVALID_PRELOGIN
            );
        }
    }
}
