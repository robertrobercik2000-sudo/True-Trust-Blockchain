#![forbid(unsafe_code)]

//! Rdzeń PQ-secure kanału P2P:
//! - KDF: 2 klucze sesji z KEM shared_secret + transcript_hash,
//! - XChaCha20-Poly1305: osobny klucz na send / recv,
//! - liczniki nonce'ów (u64) per kierunek – brak reuse nonców.
//!
//! Ten moduł NIE robi handshake'u sieciowego – tylko kryptografię kanału.

use anyhow::{anyhow, Result};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use zeroize::Zeroize;

use crate::crypto::kmac as ck;

/// 32-bajtowy hash transkryptu handshake'u.
pub type TranscriptHash = [u8; 32];

/// Symetryczny klucz sesji do XChaCha20-Poly1305.
///
/// Uwaga: pamięć jest zerowana w Drop.
#[derive(Clone, Zeroize)]
#[zeroize(drop)]
pub struct SessionKey(pub [u8; 32]);

impl SessionKey {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Z KEM shared_secret + transcript_hash wyprowadza dwa klucze:
/// - k_c2s: klient → serwer
/// - k_s2c: serwer → klient
///
/// Użyj:
/// - po stronie klienta: send = k_c2s, recv = k_s2c
/// - po stronie serwera: send = k_s2c, recv = k_c2s
pub fn derive_session_keys(
    shared_secret: &[u8],
    transcript_hash: &TranscriptHash,
) -> (SessionKey, SessionKey) {
    // KMAC256-XOF:
    // K = kmac256_xof(key = shared_secret,
    //                 custom = "TT-P2P-SESSION.v1",
    //                 data = transcript_hash,
    //                 out_len = 64)
    let out = ck::kmac256_xof(
        shared_secret,
        b"TT-P2P-SESSION.v1",
        transcript_hash,
        64,
    );

    let mut k_c2s = [0u8; 32];
    let mut k_s2c = [0u8; 32];
    k_c2s.copy_from_slice(&out[..32]);
    k_s2c.copy_from_slice(&out[32..64]);

    (SessionKey(k_c2s), SessionKey(k_s2c))
}

/// Dwukierunkowy, PQ-secure kanał po handshake.
///
/// - osobne klucze XChaCha20-Poly1305 dla send/recv,
/// - liczniki nonce'ów per kierunek (u64 → 8 bajtów w 24B nonce).
pub struct SecureChannel {
    aead_send: XChaCha20Poly1305,
    aead_recv: XChaCha20Poly1305,
    send_ctr: u64,
    recv_ctr: u64,
}

impl SecureChannel {
    /// Tworzy kanał z kluczy sesji:
    /// - `send_key` – klucz do szyfrowania wychodzących,
    /// - `recv_key` – klucz do odszyfrowywania przychodzących.
    pub fn new(send_key: SessionKey, recv_key: SessionKey) -> Self {
        let aead_send = XChaCha20Poly1305::new(send_key.0.as_slice().into());
        let aead_recv = XChaCha20Poly1305::new(recv_key.0.as_slice().into());

        Self {
            aead_send,
            aead_recv,
            send_ctr: 0,
            recv_ctr: 0,
        }
    }

    /// Tworzy nonce z 64-bitowego licznika.
    /// Pozostałe 16 bajtów zostawiamy na 0 – klucze są różne per kierunek, więc kolizji (key, nonce) nie ma.
    #[inline]
    fn make_nonce(counter: u64) -> XNonce {
        let mut n = [0u8; 24];
        n[0..8].copy_from_slice(&counter.to_le_bytes());
        *XNonce::from_slice(&n)
    }

    /// Szyfrowanie wiadomości (send → peer).
    pub fn encrypt(&mut self, plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
        let ctr = self
            .send_ctr
            .checked_add(1)
            .ok_or_else(|| anyhow!("send nonce counter overflow"))?;
        let nonce = Self::make_nonce(self.send_ctr);
        self.send_ctr = ctr;

        let ct = self
            .aead_send
            .encrypt(
                &nonce,
                Payload { msg: plaintext, aad },
            )
            .map_err(|e| anyhow!("encrypt failed: {e}"))?;
        Ok(ct)
    }

    /// Odszyfrowanie wiadomości (recv ← peer).
    pub fn decrypt(&mut self, ciphertext: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
        let ctr = self
            .recv_ctr
            .checked_add(1)
            .ok_or_else(|| anyhow!("recv nonce counter overflow"))?;
        let nonce = Self::make_nonce(self.recv_ctr);
        self.recv_ctr = ctr;

        let pt = self
            .aead_recv
            .decrypt(
                &nonce,
                Payload { msg: ciphertext, aad },
            )
            .map_err(|e| anyhow!("decrypt failed: {e}"))?;
        Ok(pt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_keys_is_deterministic() {
        let ss = b"shared_secret_example";
        let mut th = [0u8; 32];
        th[0..4].copy_from_slice(b"TEST");

        let (k1_a, k1_b) = derive_session_keys(ss, &th);
        let (k2_a, k2_b) = derive_session_keys(ss, &th);

        assert_eq!(k1_a.as_bytes(), k2_a.as_bytes());
        assert_eq!(k1_b.as_bytes(), k2_b.as_bytes());
        assert_ne!(k1_a.as_bytes(), k1_b.as_bytes()); // dwa różne kierunki
    }

    #[test]
    fn secure_channel_roundtrip() {
        let ss = b"shared_secret_example";
        let mut th = [0u8; 32];
        th[0..4].copy_from_slice(b"TEST");

        let (k_c2s, k_s2c) = derive_session_keys(ss, &th);

        // klient
        let mut chan_client = SecureChannel::new(k_c2s.clone(), k_s2c.clone());
        // serwer
        let mut chan_server = SecureChannel::new(k_s2c, k_c2s);

        let aad = b"header";
        let msg1 = b"hello from client";
        let msg2 = b"hello from server";

        // C → S
        let ct1 = chan_client.encrypt(msg1, aad).unwrap();
        let pt1 = chan_server.decrypt(&ct1, aad).unwrap();
        assert_eq!(pt1, msg1);

        // S → C
        let ct2 = chan_server.encrypt(msg2, aad).unwrap();
        let pt2 = chan_client.decrypt(&ct2, aad).unwrap();
        assert_eq!(pt2, msg2);
    }
}
