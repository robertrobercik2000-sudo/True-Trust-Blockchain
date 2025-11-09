//! TT Quantum Wallet - Full featured wallet with Falcon512 + ML-KEM
//! 
//! Integrates:
//! - Post-quantum: Falcon512 + ML-KEM (Kyber768)
//! - Traditional: Ed25519 (spend) + X25519 (scan)
//! - AEAD: AES-GCM-SIV / XChaCha20-Poly1305
//! - KDF: Argon2id + KMAC256 with pepper
//! - Backup: Shamir M-of-N secret sharing

#![forbid(unsafe_code)]

use anyhow::{anyhow, bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use zeroize::{Zeroize, Zeroizing};
use rand::rngs::OsRng;
use rand::RngCore;

// Quantum crypto
use pqcrypto_falcon::falcon512;
use pqcrypto_kyber::kyber768 as mlkem;
use pqcrypto_traits::sign::{PublicKey as PQPublicKey, SecretKey as PQSecretKey};
use pqcrypto_traits::kem::{PublicKey as PQKemPublicKey, SecretKey as PQKemSecretKey};

// Traditional crypto
use ed25519_dalek::{SigningKey as Ed25519Secret, VerifyingKey as Ed25519Public};
use x25519_dalek::{PublicKey as X25519Public, StaticSecret as X25519Secret};

// Our modules  
use crate::crypto::kmac as ck;  // ✅ POPRAWIONE: crypto::kmac ma derive_key

/* =========================================================================================
 * CONSTANTS
 * ====================================================================================== */

pub const WALLET_VERSION_QUANTUM: u32 = 5; // v5 = quantum support
pub const BECH32_HRP: &str = "tt";

/* =========================================================================================
 * Wallet Structures
 * ====================================================================================== */

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AeadKind { AesGcmSiv, XChaCha20 }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PepperPolicy { None, OsLocal }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KdfHeader {
    pub kind: KdfKind,
    pub info: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum KdfKind {
    Kmac256V1 { salt32: [u8; 32] },
    #[cfg(feature = "tt-full")]
    Argon2idV1 { mem_kib: u32, time_cost: u32, lanes: u32, salt32: [u8; 32] },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WalletHeader {
    pub version: u32,
    pub kdf: KdfHeader,
    pub aead: AeadKind,
    pub nonce12: [u8; 12],
    pub nonce24_opt: Option<[u8; 24]>,
    pub padding_block: u16,
    pub pepper: PepperPolicy,
    pub wallet_id: [u8; 16],
    pub quantum_enabled: bool,  // ✅ NOWE
}

/// V3 Payload with quantum keys
#[derive(Clone, Serialize, Deserialize, Zeroize)]
#[zeroize(drop)]
pub struct WalletSecretPayloadV3 {
    pub master32: [u8; 32],
    
    // Traditional keys (always present)
    pub ed25519_spend_sk: [u8; 32],
    pub x25519_scan_sk: [u8; 32],
    
    // ✅ QUANTUM KEYS (optional, only if quantum_enabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub falcon_sk_bytes: Option<Vec<u8>>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub falcon_pk_bytes: Option<Vec<u8>>,  // ✅ DODANE: Musimy przechowywać PK osobno!
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mlkem_sk_bytes: Option<Vec<u8>>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mlkem_pk_bytes: Option<Vec<u8>>,   // ✅ DODANE: Również dla ML-KEM
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WalletFile {
    pub header: WalletHeader,
    pub enc: Vec<u8>,  // encrypted WalletSecretPayloadV3
}

/* =========================================================================================
 * Keyset (full keys)
 * ====================================================================================== */

#[derive(Clone)]
pub struct Keyset {
    // Traditional (always)
    pub spend_sk: Ed25519Secret,
    pub spend_pk: Ed25519Public,
    pub scan_sk: X25519Secret,
    pub scan_pk: X25519Public,
    
    // ✅ Quantum (optional)
    pub falcon_sk: Option<falcon512::SecretKey>,
    pub falcon_pk: Option<falcon512::PublicKey>,
    pub mlkem_sk: Option<mlkem::SecretKey>,
    pub mlkem_pk: Option<mlkem::PublicKey>,
}

impl Keyset {
    /// Create from master32 (traditional only)
    pub fn from_master_v2(master32: &[u8; 32]) -> Self {
        let spend32 = ck::kmac256_derive_key(master32, b"TT-SPEND.v1", b"seed");
        let scan32 = ck::kmac256_derive_key(master32, b"TT-SCAN.v1", b"seed");
        
        let spend_sk = Ed25519Secret::from_bytes(&spend32);
        let spend_pk = Ed25519Public::from(&spend_sk);
        let scan_sk = X25519Secret::from(scan32);
        let scan_pk = X25519Public::from(&scan_sk);
        
        Self {
            spend_sk,
            spend_pk,
            scan_sk,
            scan_pk,
            falcon_sk: None,
            falcon_pk: None,
            mlkem_sk: None,
            mlkem_pk: None,
        }
    }
    
    /// Create from payload V3 (with optional quantum)
    pub fn from_payload_v3(payload: &WalletSecretPayloadV3) -> Result<Self> {
        let spend_sk = Ed25519Secret::from_bytes(&payload.ed25519_spend_sk);
        let spend_pk = Ed25519Public::from(&spend_sk);
        let scan_sk = X25519Secret::from(payload.x25519_scan_sk);
        let scan_pk = X25519Public::from(&scan_sk);
        
        let (falcon_sk, falcon_pk, mlkem_sk, mlkem_pk) = if let (Some(ref f_sk_bytes), Some(ref f_pk_bytes)) = 
            (&payload.falcon_sk_bytes, &payload.falcon_pk_bytes) 
        {
            let f_sk = falcon512::SecretKey::from_bytes(f_sk_bytes)
                .map_err(|_| anyhow!("invalid Falcon SK"))?;
            let f_pk = falcon512::PublicKey::from_bytes(f_pk_bytes)
                .map_err(|_| anyhow!("invalid Falcon PK"))?;
            
            let (m_sk, m_pk) = if let (Some(ref m_sk_bytes), Some(ref m_pk_bytes)) = 
                (&payload.mlkem_sk_bytes, &payload.mlkem_pk_bytes)
            {
                let sk = mlkem::SecretKey::from_bytes(m_sk_bytes)
                    .map_err(|_| anyhow!("invalid ML-KEM SK"))?;
                let pk = mlkem::PublicKey::from_bytes(m_pk_bytes)
                    .map_err(|_| anyhow!("invalid ML-KEM PK"))?;
                (Some(sk), Some(pk))
            } else {
                (None, None)
            };
            
            (Some(f_sk), Some(f_pk), m_sk, m_pk)
        } else {
            (None, None, None, None)
        };
        
        Ok(Self {
            spend_sk,
            spend_pk,
            scan_sk,
            scan_pk,
            falcon_sk,
            falcon_pk,
            mlkem_sk,
            mlkem_pk,
        })
    }
    
    pub fn is_quantum(&self) -> bool {
        self.falcon_sk.is_some() && self.mlkem_sk.is_some()
    }
}

/* =========================================================================================
 * Wallet Creation
 * ====================================================================================== */

pub fn create_wallet_v3(master32: [u8; 32], quantum: bool) -> Result<WalletSecretPayloadV3> {
    // Traditional keys (always derive)
    let spend32 = ck::kmac256_derive_key(&master32, b"TT-SPEND.v1", b"seed");
    let scan32 = ck::kmac256_derive_key(&master32, b"TT-SCAN.v1", b"seed");
    
    // ✅ Quantum keys (generate fresh, store in wallet)
    let (falcon_sk_bytes, falcon_pk_bytes, mlkem_sk_bytes, mlkem_pk_bytes) = if quantum {
        let (f_pk, f_sk) = falcon512::keypair();
        let (m_pk, m_sk) = mlkem::keypair();
        
        (
            Some(f_sk.as_bytes().to_vec()),
            Some(f_pk.as_bytes().to_vec()),  // ✅ DODANE
            Some(m_sk.as_bytes().to_vec()),
            Some(m_pk.as_bytes().to_vec()),  // ✅ DODANE
        )
    } else {
        (None, None, None, None)
    };
    
    Ok(WalletSecretPayloadV3 {
        master32,
        ed25519_spend_sk: spend32,
        x25519_scan_sk: scan32,
        falcon_sk_bytes,
        falcon_pk_bytes,  // ✅ DODANE
        mlkem_sk_bytes,
        mlkem_pk_bytes,   // ✅ DODANE
    })
}

/* =========================================================================================
 * Address (bech32)
 * ====================================================================================== */

pub fn bech32_addr(scan_pk: &X25519Public, spend_pk: &Ed25519Public) -> Result<String> {
    use bech32::{Hrp, Bech32m};
    let mut payload = Vec::with_capacity(65);
    payload.push(0x01); // version
    payload.extend_from_slice(scan_pk.as_bytes());
    payload.extend_from_slice(spend_pk.as_bytes());
    let hrp = Hrp::parse(BECH32_HRP).map_err(|e| anyhow!("bad HRP: {e}"))?;
    Ok(bech32::encode::<Bech32m>(hrp, &payload).map_err(|e| anyhow!("bech32 encode: {e}"))?)
}

pub fn bech32_addr_quantum(
    scan_pk: &X25519Public,
    spend_pk: &Ed25519Public,
    falcon_pk: &falcon512::PublicKey,
    mlkem_pk: &mlkem::PublicKey,
) -> Result<String> {
    use bech32::{Hrp, Bech32m};
    let mut payload = Vec::with_capacity(2048);
    payload.push(0x02); // version 2 = quantum
    payload.extend_from_slice(scan_pk.as_bytes());
    payload.extend_from_slice(spend_pk.as_bytes());
    payload.extend_from_slice(falcon_pk.as_bytes());
    payload.extend_from_slice(mlkem_pk.as_bytes());
    let hrp = Hrp::parse(BECH32_HRP).map_err(|e| anyhow!("bad HRP: {e}"))?;
    Ok(bech32::encode::<Bech32m>(hrp, &payload).map_err(|e| anyhow!("bech32 encode: {e}"))?)
}

/* =========================================================================================
 * Helpers
 * ====================================================================================== */

pub fn random_wallet_id() -> [u8; 16] {
    let mut id = [0u8; 16];
    OsRng.fill_bytes(&mut id);
    id
}

pub fn random_nonce12() -> [u8; 12] {
    let mut n = [0u8; 12];
    OsRng.fill_bytes(&mut n);
    n
}

pub fn random_nonce24() -> [u8; 24] {
    let mut n = [0u8; 24];
    OsRng.fill_bytes(&mut n);
    n
}

pub fn random_master() -> [u8; 32] {
    let mut m = [0u8; 32];
    OsRng.fill_bytes(&mut m);
    m
}

/* =========================================================================================
 * Tests
 * ====================================================================================== */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyset_v2_compat() {
        let master = [0x42u8; 32];
        let ks = Keyset::from_master_v2(&master);
        assert!(!ks.is_quantum());
        assert!(ks.falcon_sk.is_none());
        assert!(ks.mlkem_sk.is_none());
    }

    #[test]
    fn test_wallet_quantum_creation() {
        let master = random_master();
        let payload = create_wallet_v3(master, true).unwrap();
        
        assert!(payload.falcon_sk_bytes.is_some());
        assert!(payload.mlkem_sk_bytes.is_some());
        
        let ks = Keyset::from_payload_v3(&payload).unwrap();
        assert!(ks.is_quantum());
    }

    #[test]
    fn test_wallet_traditional_creation() {
        let master = random_master();
        let payload = create_wallet_v3(master, false).unwrap();
        
        assert!(payload.falcon_sk_bytes.is_none());
        assert!(payload.mlkem_sk_bytes.is_none());
        
        let ks = Keyset::from_payload_v3(&payload).unwrap();
        assert!(!ks.is_quantum());
    }
}
