//! KMAC + Falcon512 + ML-KEM(kyber768) integration for quantum-safe keysearch
//! **NOTE:** This is LEGACY module - may use Falcon incorrectly!
//! Use quantum_hint_v2 for corrected implementation.
#![forbid(unsafe_code)]

use crate::crypto::kmac::{kmac256_derive_key, kmac256_xof_fill};
use crate::keysearch::{HintPayloadV1, DecodedHint};
use chacha20poly1305::{XChaCha20Poly1305, aead::{Aead, KeyInit}, XNonce};
use lru::LruCache;
use pqcrypto_falcon::falcon512;
use pqcrypto_kyber::kyber768 as mlkem;
use pqcrypto_traits::sign::{PublicKey as PQPublicKey, SignedMessage as PQSignedMessage};
use pqcrypto_traits::kem::{Ciphertext as PQCiphertext, SharedSecret as PQSharedSecret};
use serde::{Deserialize, Serialize};
use std::num::NonZeroUsize;
use std::time::{SystemTime, UNIX_EPOCH};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret as X25519Secret};
use zeroize::{Zeroize, Zeroizing};

/* ===== TYPE ALIASES ===== */

type FalconPublicKey = falcon512::PublicKey;
type FalconSecretKey = falcon512::SecretKey;
type FalconSignedMessage = falcon512::SignedMessage;

type MlkemPublicKey = mlkem::PublicKey;
type MlkemSecretKey = mlkem::SecretKey;

/* ===== PUBLIC HINT STRUCTURES ===== */

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuantumSafeHint {
    /// ML-KEM (Kyber768) ciphertext – do decapsulacji po stronie odbiorcy
    pub kem_ct: Vec<u8>,
    /// Efemeryczny publiczny X25519 (hybryda; przyspiesza i twardzi tajemnicę)
    pub x25519_eph_pub: [u8; 32],
    /// Falcon SignedMessage = SIG || TRANSCRIPT (podpis nad transkryptem)
    pub falcon_signed_msg: Vec<u8>,
    /// Publiczny klucz Falcon nadawcy (do weryfikacji podpisu)
    pub sender_falcon_pk: Vec<u8>,
    /// AEAD ciphertext payloadu (XChaCha20-Poly1305, AAD = transcript)
    pub enc_payload: Vec<u8>,
    /// anty-replay / porządkowanie
    pub timestamp: u64,
    /// epoka rotacji kluczy
    pub epoch: u64,
}

#[derive(Clone, Debug)]
pub struct QuantumFoundNote {
    pub index: usize,
    pub c_out: [u8; 32],
    pub k_search: [u8; 32],
    pub falcon_verified: bool,  // zawsze true w ścieżce PQC
    pub quantum_safe: bool,     // zawsze true w ścieżce PQC
}

/* ===== KEY MANAGER (epoki) ===== */

pub struct FalconKeyManager {
    current_epoch: u64,
    epoch_duration: u64,
}

impl FalconKeyManager {
    pub fn new() -> Self {
        Self { current_epoch: 0, epoch_duration: 86_400 }
    }
    pub fn get_current_epoch(&self) -> u64 { self.current_epoch }
    pub fn rotate_epoch(&mut self) { self.current_epoch = self.current_epoch.saturating_add(1); }

    /// Akceptujemy hint z bieżącej lub poprzedniej epoki (skew)
    pub fn verify_epoch(&self, hint: &QuantumSafeHint) -> bool {
        let e = self.current_epoch;
        hint.epoch == e || hint.epoch.saturating_add(1) == e
    }
}

/* ===== CONTEXT ===== */

pub struct QuantumKeySearchCtx {
    // Tożsamość PQC do podpisu (nadawcy) — w realnym systemie dostarczane z zewnątrz
    falcon_sk: FalconSecretKey,
    falcon_pk: FalconPublicKey,

    // Odbiorca (wallet): para ML-KEM – do decapsulacji
    kem_sk: MlkemSecretKey,
    kem_pk: MlkemPublicKey,

    // X25519 (statyczny po stronie odbiorcy; nadawca robi efemeryczny)
    x25519_secret: Zeroizing<[u8; 32]>,

    // Epoki / polityka czasu
    key_manager: FalconKeyManager,

    // Cache (opcjonalny)
    cache: QuantumSearchCache,
}

impl QuantumKeySearchCtx {
    /// Inicjalizacja kontekstu odbiorcy/nadawcy — w przykładzie generujemy klucze lokalnie.
    /// W praktyce: przekaż gotowe pary (Falcon, ML-KEM, X25519).
    pub fn new(master_seed: [u8; 32]) -> Result<Self, FalconError> {
        // Uwaga: API PQC z pqcrypto generuje losowo – deterministyka z seedem wymaga osobnej warstwy RNG.
        let (falcon_pk, falcon_sk) = falcon512::keypair();
        let (kem_pk, kem_sk) = mlkem::keypair();

        let x25519_secret = Zeroizing::new(kmac256_derive_key(&master_seed, b"X25519_SESSION_KEY", b"key_derivation"));

        Ok(Self {
            falcon_sk,
            falcon_pk,
            kem_sk,
            kem_pk,
            x25519_secret,
            key_manager: FalconKeyManager::new(),
            cache: QuantumSearchCache::new(),
        })
    }

    /* --- public helpers --- */

    pub fn kem_public_key(&self) -> &MlkemPublicKey { &self.kem_pk }
    pub fn falcon_public_key(&self) -> &FalconPublicKey { &self.falcon_pk }
    pub fn x25519_public_key(&self) -> X25519PublicKey {
        let sk = X25519Secret::from(*self.x25519_secret);
        X25519PublicKey::from(&sk)
    }
    pub fn epoch(&self) -> u64 { self.key_manager.get_current_epoch() }

    /* --- budowa hinta (nadawca) --- */

    /// Buduje quantum-safe hint: ML-KEM + X25519 (hybryda), podpis Falcon, AEAD.
    pub fn build_quantum_hint(
        &self,
        recipient_kem_pk: &MlkemPublicKey,
        recipient_x25519_pk: &X25519PublicKey,
        c_out: &[u8; 32],
        payload: &HintPayloadV1,
    ) -> Result<QuantumSafeHint, FalconError> {
        // 1) ID transakcji (KMAC) – wiążemy z c_out i payloadem
        let transaction_id = Self::derive_transaction_id(c_out, payload);

        // 2) Efemeryczny X25519 po stronie nadawcy (z KMAC)
        let eph_x_secret = kmac256_derive_key(&*self.x25519_secret, b"X25519_EPHEMERAL", &transaction_id);
        let eph_sk = X25519Secret::from(eph_x_secret);
        let x25519_eph_pub = X25519PublicKey::from(&eph_sk).to_bytes();

        // 3) DH z publicznym odbiorcy
        let dh = eph_sk.diffie_hellman(recipient_x25519_pk).to_bytes();

        // 4) PQC KEM: encapsulate → (ss, ct) - UWAGA: kolejność w pqcrypto-kyber!
        let (kem_ss, kem_ct) = mlkem::encapsulate(recipient_kem_pk);

        // 5) Hybrydowy sekret: ss_h = KMAC(ss || dh, "QH/HYBRID", c_out)
        let kem_ss_bytes = <mlkem::SharedSecret as PQSharedSecret>::as_bytes(&kem_ss); // ✅ UFCS
        let kem_ct_bytes = <mlkem::Ciphertext as PQCiphertext>::as_bytes(&kem_ct);     // ✅ UFCS
        let mut ss_input = Vec::with_capacity(kem_ss_bytes.len() + dh.len());
        ss_input.extend_from_slice(kem_ss_bytes);
        ss_input.extend_from_slice(dh.as_ref());
        let ss_h = kmac256_derive_key(&ss_input, b"QH/HYBRID", c_out);

        // 6) Transcript + podpis Falconem nadawcy
        let epoch = self.key_manager.get_current_epoch();
        let timestamp = current_timestamp();
        let tr = transcript(epoch, timestamp, c_out, kem_ct_bytes, &x25519_eph_pub, self.falcon_pk.as_bytes());
        let sm = falcon512::sign(&tr, &self.falcon_sk);
        let falcon_signed_msg = sm.as_bytes().to_vec();

        // 7) AEAD (XChaCha20-Poly1305) nad payloadem, AAD = transcript
        let enc_payload = encrypt_payload_aead(&ss_h, &tr, payload)?;

        Ok(QuantumSafeHint {
            kem_ct: kem_ct_bytes.to_vec(), // ✅ używamy wcześniej wyliczonego
            x25519_eph_pub,
            falcon_signed_msg,
            sender_falcon_pk: self.falcon_pk.as_bytes().to_vec(),
            enc_payload,
            timestamp,
            epoch,
        })
    }

    /* --- weryfikacja/dekrypt (odbiorca) --- */

    /// Weryfikuje i dekoduje quantum-safe hint.
    /// Zwraca (DecodedHint, true) w przypadku sukcesu.
    pub fn verify_quantum_hint(
        &self,
        hint: &QuantumSafeHint,
        c_out: &[u8; 32],
        max_skew_secs: u64, // np. 2h = 7200
    ) -> Option<(DecodedHint, bool)> {
        // 0) Epoka + anty-replay (czas)
        if !self.key_manager.verify_epoch(hint) { return None; }
        let now = current_timestamp();
        if now.saturating_sub(hint.timestamp) > max_skew_secs { return None; }

        // 1) Transcript (musi być identyczny jak przy podpisie)
        let tr = transcript(hint.epoch, hint.timestamp, c_out, &hint.kem_ct, &hint.x25519_eph_pub, &hint.sender_falcon_pk);

        // 2) Weryfikacja podpisu Falcon – używamy klucza nadawcy z hinta
        let sender_pk = FalconPublicKey::from_bytes(&hint.sender_falcon_pk).ok()?;
        let sm = FalconSignedMessage::from_bytes(&hint.falcon_signed_msg).ok()?;
        let opened = falcon512::open(&sm, &sender_pk).ok()?;
        if opened != tr { return None; } // ✅ POPRAWIONE: falcon open() zwraca Vec<u8>

        // 3) Decapsulation ML-KEM odbiorcy
        let kem_ct = mlkem::Ciphertext::from_bytes(&hint.kem_ct).ok()?; // ✅ używa traitu PQCiphertext
        let kem_ss = mlkem::decapsulate(&kem_ct, &self.kem_sk);

        // 4) DH z X25519 (nasz statyczny vs ich efemeryczny)
        let eph = X25519PublicKey::from(hint.x25519_eph_pub);
        let sk = X25519Secret::from(*self.x25519_secret);
        let dh = sk.diffie_hellman(&eph).to_bytes();

        // 5) Hybryda sekretu
        let mut ss_input = Vec::new();
        ss_input.extend_from_slice(<mlkem::SharedSecret as PQSharedSecret>::as_bytes(&kem_ss)); // ✅ UFCS
        ss_input.extend_from_slice(dh.as_ref());
        let ss_h = kmac256_derive_key(&ss_input, b"QH/HYBRID", c_out);

        // 6) AEAD decrypt
        let payload = decrypt_payload_aead(&ss_h, &tr, &hint.enc_payload)?;
        let decoded = DecodedHint {
            r_blind: payload.r_blind,
            value: Some(payload.value),
            memo_items: crate::keysearch::tlv::decode(&payload.memo),
        };

        Some((decoded, true))
    }

    /* --- batch scanning --- */

    pub fn scan_quantum_safe<'a, I>(&self, outputs: I) -> Vec<QuantumFoundNote>
    where
        I: IntoIterator<Item = (usize, &'a [u8; 32], &'a QuantumSafeHint)>,
    {
        let mut hits = Vec::new();
        for (idx, c_out, hint) in outputs {
            if let Some((decoded, verified)) = self.verify_quantum_hint(hint, c_out, 7_200) {
                let k_search = self.quantum_k_search(c_out, hint, &decoded);
                hits.push(QuantumFoundNote {
                    index: idx,
                    c_out: *c_out,
                    k_search,
                    falcon_verified: verified,
                    quantum_safe: verified,
                });
            }
        }
        hits
    }

    /* ===== INTERNAL: k_search, transcript, tx_id ===== */

    fn quantum_k_search(
        &self,
        c_out: &[u8; 32],
        hint: &QuantumSafeHint,
        decoded: &DecodedHint,
    ) -> [u8; 32] {
        // Odzyskaj ten sam hybrydowy sekret jak w verify() (powtórka – prostsza zależność)
        let kem_ct = mlkem::Ciphertext::from_bytes(&hint.kem_ct).expect("kem_ct bytes");
        let kem_ss = mlkem::decapsulate(&kem_ct, &self.kem_sk);

        let eph = X25519PublicKey::from(hint.x25519_eph_pub);
        let sk = X25519Secret::from(*self.x25519_secret);
        let dh = sk.diffie_hellman(&eph).to_bytes();

        let mut ss_input = Vec::new();
        ss_input.extend_from_slice(<mlkem::SharedSecret as PQSharedSecret>::as_bytes(&kem_ss)); // ✅ UFCS
        ss_input.extend_from_slice(dh.as_ref());
        let ss_h = kmac256_derive_key(&ss_input, b"QH/HYBRID", c_out);

        let mut custom = Vec::new();
        custom.extend_from_slice(c_out);
        custom.extend_from_slice(&decoded.r_blind);
        custom.extend_from_slice(&hint.sender_falcon_pk);

        kmac256_derive_key(&ss_h, b"QH/KSEARCH", &custom)
    }

    fn derive_transaction_id(c_out: &[u8; 32], payload: &HintPayloadV1) -> [u8; 32] {
        let mut tx_input = Vec::new();
        tx_input.extend_from_slice(c_out);
        tx_input.extend_from_slice(&payload.r_blind);
        tx_input.extend_from_slice(&payload.value.to_le_bytes());
        kmac256_derive_key(c_out, b"QH/TRANSACTION_ID", &tx_input)
    }
}

/* ===== TRANSCRIPT ===== */

fn transcript(
    epoch: u64,
    timestamp: u64,
    c_out: &[u8; 32],
    kem_ct: &[u8],
    x25519_eph_pub: &[u8; 32],
    sender_falcon_pk: &[u8],
) -> Vec<u8> {
    let mut t = Vec::with_capacity(16 + 32 + kem_ct.len() + 32 + sender_falcon_pk.len());
    t.extend_from_slice(b"QHINT.v1");
    t.extend_from_slice(c_out);
    t.extend_from_slice(&epoch.to_le_bytes());
    t.extend_from_slice(&timestamp.to_le_bytes());
    t.extend_from_slice(&(kem_ct.len() as u32).to_le_bytes());
    t.extend_from_slice(kem_ct);
    t.extend_from_slice(x25519_eph_pub);
    t.extend_from_slice(sender_falcon_pk);
    t
}

/* ===== AEAD ===== */

fn encrypt_payload_aead(
    ss_h: &[u8; 32],
    aad: &[u8],
    payload: &HintPayloadV1,
) -> Result<Vec<u8>, FalconError> {
    let key = kmac256_derive_key(ss_h, b"QH/AEAD/Key", b"");
    let mut nonce24 = [0u8; 24];
    kmac256_xof_fill(ss_h, b"QH/AEAD/Nonce24", b"", &mut nonce24); // ✅ POPRAWIONE

    let cipher = XChaCha20Poly1305::new_from_slice(&key).map_err(|_| FalconError::SerializationFailed)?;
    let pt = bincode::serialize(payload).map_err(|_| FalconError::SerializationFailed)?;
    let nonce = XNonce::from(nonce24); // ✅ POPRAWIONE: from zamiast from_slice
    let ct = cipher.encrypt(&nonce.into(), chacha20poly1305::aead::Payload { msg: &pt, aad })
                   .map_err(|_| FalconError::SerializationFailed)?;
    Ok(ct)
}

fn decrypt_payload_aead(ss_h: &[u8; 32], aad: &[u8], ct: &[u8]) -> Option<HintPayloadV1> {
    let key = kmac256_derive_key(ss_h, b"QH/AEAD/Key", b"");
    let mut nonce24 = [0u8; 24];
    kmac256_xof_fill(ss_h, b"QH/AEAD/Nonce24", b"", &mut nonce24); // ✅ POPRAWIONE

    let cipher = XChaCha20Poly1305::new_from_slice(&key).ok()?;
    let nonce = XNonce::from(nonce24); // ✅ POPRAWIONE: from zamiast from_slice
    let pt = cipher.decrypt(&nonce.into(), chacha20poly1305::aead::Payload { msg: ct, aad }).ok()?;
    bincode::deserialize(&pt).ok()
}

/* ===== CACHE ===== */

struct QuantumSearchCache {
    #[allow(dead_code)]
    verified_hints: LruCache<[u8; 32], bool>,
    #[allow(dead_code)]
    epoch_keys: LruCache<u64, (Vec<u8>, Vec<u8>)>,
}

impl QuantumSearchCache {
    fn new() -> Self {
        Self {
            verified_hints: LruCache::new(NonZeroUsize::new(1000).unwrap()),
            epoch_keys: LruCache::new(NonZeroUsize::new(5).unwrap()),
        }
    }
}

/* ===== ERRORS ===== */

#[derive(Debug, thiserror::Error)]
pub enum FalconError {
    #[error("Falcon key generation failed")]
    KeyGenerationFailed,
    #[error("Falcon signing failed")]
    SigningFailed,
    #[error("Falcon verification failed")]
    VerificationFailed,
    #[error("Invalid key")]
    InvalidKey,
    #[error("Serialization failed")]
    SerializationFailed,
}

/* ===== UTILS ===== */

fn current_timestamp() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

/* ===== TESTS ===== */

#[cfg(test)]
mod tests {
    use super::*;
    use x25519_dalek::PublicKey as X25519PublicKey;

    fn mk_payload(v: u64) -> HintPayloadV1 {
        HintPayloadV1 { r_blind: [0x11; 32], value: v, memo: Vec::new() }
    }

    #[test]
    fn roundtrip_hint_pqc() {
        let seed = [0x42u8; 32];
        let ctx = QuantumKeySearchCtx::new(seed).unwrap();

        let c_out = [0xAAu8; 32];
        let payload = mk_payload(12345);

        let recip_kem_pk = ctx.kem_public_key().clone(); // odbiorcą jest "ctx"
        let recip_x_pub = ctx.x25519_public_key();

        let hint = ctx.build_quantum_hint(&recip_kem_pk, &recip_x_pub, &c_out, &payload).unwrap();
        let out = ctx.verify_quantum_hint(&hint, &c_out, 7200);
        assert!(out.is_some());
        let (dec, verified) = out.unwrap();
        assert!(verified);
        assert_eq!(dec.value, Some(12345));
        assert_eq!(dec.r_blind, payload.r_blind);
    }

    #[test]
    fn ksearch_derivation_consistency() {
        let seed = [0x55u8; 32]; // ✅ POPRAWIONE (było: [0x55u8; 3 \n 2])
        let ctx = QuantumKeySearchCtx::new(seed).unwrap();
        let c_out = [0xBBu8; 32];
        let payload = mk_payload(7);

        let recip_kem_pk = ctx.kem_public_key().clone();
        let recip_x_pub = ctx.x25519_public_key();

        let hint = ctx.build_quantum_hint(&recip_kem_pk, &recip_x_pub, &c_out, &payload).unwrap();
        let (dec, _) = ctx.verify_quantum_hint(&hint, &c_out, 7200).expect("verify");

        let k1 = ctx.quantum_k_search(&c_out, &hint, &dec);
        let k2 = ctx.quantum_k_search(&c_out, &hint, &dec);
        assert_eq!(k1, k2);
        assert_ne!(k1, [0u8; 32]);
    }
}
