//! `BackendKeyData` keys and the `CancelRequest`s that use them.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ring::rand::{SecureRandom, SystemRandom};
use tokio::sync::watch;

/// The keys of the live connections.
#[derive(Default)]
pub(crate) struct CancelRegistry {
    entries: Mutex<HashMap<u32, Entry>>,
}

struct Entry {
    secret: [u8; 4],
    signal: watch::Sender<u64>,
}

/// The key of one connection; dropping it forgets the key.
pub(crate) struct CancelKey {
    registry: Arc<CancelRegistry>,
    pub(crate) process_id: u32,
    pub(crate) secret: u32,
    pub(crate) signal: watch::Receiver<u64>,
}

impl Drop for CancelKey {
    fn drop(&mut self) {
        self.registry
            .entries
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .remove(&self.process_id);
    }
}

fn random_u32(random: &SystemRandom) -> u32 {
    let mut bytes = [0_u8; 4];
    random
        .fill(&mut bytes)
        .expect("the system random source is available");
    u32::from_be_bytes(bytes)
}

impl CancelRegistry {
    /// Registers a connection under a fresh random process identifier and
    /// secret.
    pub(crate) fn register(registry: &Arc<Self>) -> CancelKey {
        let random = SystemRandom::new();
        let mut entries = registry
            .entries
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let process_id = loop {
            // Positive, as clients read it into a signed integer.
            let candidate = random_u32(&random) & 0x7fff_ffff;
            if candidate != 0 && !entries.contains_key(&candidate) {
                break candidate;
            }
        };
        let secret = random_u32(&random);
        let (signal, receiver) = watch::channel(0);
        entries.insert(
            process_id,
            Entry {
                secret: secret.to_be_bytes(),
                signal,
            },
        );
        CancelKey {
            registry: Arc::clone(registry),
            process_id,
            secret,
            signal: receiver,
        }
    }

    /// Signals the connection a matching key names; an unknown key is
    /// ignored, as PostgreSQL ignores it.
    pub(crate) fn cancel(&self, process_id: u32, secret: &[u8]) -> bool {
        let entries = self
            .entries
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let Some(entry) = entries.get(&process_id) else {
            return false;
        };
        let matches = secret.len() == 4
            && secret
                .iter()
                .zip(entry.secret)
                .fold(0_u8, |difference, (left, right)| {
                    difference | (left ^ right)
                })
                == 0;
        if matches {
            entry.signal.send_modify(|generation| *generation += 1);
        }
        matches
    }
}
