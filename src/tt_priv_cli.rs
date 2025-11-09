//! TT Private CLI - Full quantum wallet implementation
//! 
//! Features:
//! - Wallet v5 with optional PQC (Falcon512 + ML-KEM/Kyber768)
//! - Argon2id KDF with OS pepper
//! - AEAD: AES-GCM-SIV / XChaCha20-Poly1305
//! - Shamir M-of-N secret sharing
//! - Quantum-safe keysearch

#![forbid(unsafe_code)]

pub use anyhow::{anyhow, bail, ensure, Context, Result};
pub use clap::{Parser, Subcommand, ValueEnum, Args};
pub use rand::rngs::OsRng;
pub use rand::RngCore;
pub use rpassword::prompt_password;
pub use serde::{Deserialize, Serialize};
pub use bincode::Options;
pub use std::fs::{self, OpenOptions};
pub use std::io::Write;
pub use std::path::{Path, PathBuf};
pub use zeroize::{Zeroize, Zeroizing};
pub use sha3::{Shake256, digest::{Update, ExtendableOutput, XofReader}};

// crypto / keys
pub use aes_gcm_siv::{Aes256GcmSiv, Nonce as Nonce12Siv};
pub use chacha20poly1305::{XChaCha20Poly1305, XNonce as Nonce24};
pub use ed25519_dalek::{SigningKey as Ed25519Secret, VerifyingKey as Ed25519Public};
pub use x25519_dalek::{PublicKey as X25519Public, StaticSecret as X25519Secret};
pub use argon2::{Algorithm, Argon2, Params, Version};
pub use dirs::config_dir;

// PQC
pub use pqcrypto_falcon::falcon512;
pub use pqcrypto_kyber::kyber768 as mlkem;
pub use pqcrypto_traits::sign::{PublicKey as PQPublicKey, SecretKey as PQSecretKey};
pub use pqcrypto_traits::kem::{PublicKey as PQKemPublicKey, SecretKey as PQKemSecretKey};

// Shamir
pub use sharks::{Sharks, Share};

// Our crypto
pub use crate::crypto_kmac as ck;

// Constants
pub const WALLET_VERSION: u32 = 5;
pub const BECH32_HRP: &str = "tt";
pub const WALLET_MAX_SIZE: u64 = 1 << 20;

// CLI args
#[derive(Parser, Debug)]
#[command(name = "tt_priv_cli", version, author)]
pub struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(ValueEnum, Clone, Debug)]
enum AeadFlag { GcmSiv, XChaCha20 }

#[derive(ValueEnum, Clone, Debug)]
enum PepperFlag { None, OsLocal }

#[derive(Subcommand, Debug)]
enum Cmd {
    WalletInit {
        #[arg(long)] file: PathBuf,
        #[arg(long, default_value_t = true)] argon2: bool,
        #[arg(long, value_enum, default_value_t = AeadFlag::GcmSiv)] aead: AeadFlag,
        #[arg(long, value_enum, default_value_t = PepperFlag::OsLocal)] pepper: PepperFlag,
        #[arg(long, default_value_t = 1024)] pad_block: u16,
        #[arg(long, default_value_t = false)] quantum: bool,
    },
    WalletAddr { #[arg(long)] file: PathBuf },
    WalletExport { #[arg(long)] file: PathBuf, #[arg(long)] secret: bool, #[arg(long)] out: Option<PathBuf> },
    WalletRekey {
        #[arg(long)] file: PathBuf,
        #[arg(long, default_value_t = true)] argon2: bool,
        #[arg(long, value_enum, default_value_t = AeadFlag::GcmSiv)] aead: AeadFlag,
        #[arg(long, value_enum, default_value_t = PepperFlag::OsLocal)] pepper: PepperFlag,
        #[arg(long, default_value_t = 1024)] pad_block: u16,
    },
    ShardsCreate {
        #[arg(long)] file: PathBuf,
        #[arg(long)] out_dir: PathBuf,
        #[arg(long)] m: u8,
        #[arg(long)] n: u8,
        #[arg(long, default_value_t=false)] per_share_pass: bool,
    },
    ShardsRecover {
        #[arg(long, value_delimiter=',')] input: Vec<PathBuf>,
        #[arg(long)] out: PathBuf,
        #[arg(long, default_value_t = true)] argon2: bool,
        #[arg(long, value_enum, default_value_t = AeadFlag::GcmSiv)] aead: AeadFlag,
        #[arg(long, value_enum, default_value_t = PepperFlag::OsLocal)] pepper: PepperFlag,
        #[arg(long, default_value_t = 1024)] pad_block: u16,
    },
}

// Wallet types
#[derive(Clone, Debug, Serialize, Deserialize)]
enum AeadKind { AesGcmSiv, XChaCha20 }

#[derive(Clone, Debug, Serialize, Deserialize)]
enum PepperPolicy { None, OsLocal }

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WalletHeader {
    version: u32,
    kdf: KdfHeader,
    aead: AeadKind,
    nonce12: [u8; 12],
    nonce24_opt: Option<[u8; 24]>,
    padding_block: u16,
    pepper: PepperPolicy,
    wallet_id: [u8; 16],
    quantum_enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct KdfHeader {
    kind: KdfKind,
    info: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum KdfKind {
    Kmac256V1 { salt32: [u8; 32] },
    Argon2idV1 { mem_kib: u32, time_cost: u32, lanes: u32, salt32: [u8; 32] },
}

#[derive(Clone, Serialize, Deserialize, Zeroize)]
#[zeroize(drop)]
struct WalletSecretPayloadV3 {
    master32: [u8; 32],
    ed25519_spend_sk: [u8; 32],
    x25519_scan_sk: [u8; 32],
    #[serde(skip_serializing_if = "Option::is_none")] falcon_sk_bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")] falcon_pk_bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")] mlkem_sk_bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")] mlkem_pk_bytes: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WalletFile {
    header: WalletHeader,
    enc: Vec<u8>,
}

pub struct Keyset {
    pub spend_sk: Ed25519Secret,
    pub spend_pk: Ed25519Public,
    pub scan_sk: X25519Secret,
    pub scan_pk: X25519Public,
    pub falcon_sk: Option<falcon512::SecretKey>,
    pub falcon_pk: Option<falcon512::PublicKey>,
    pub mlkem_sk: Option<mlkem::SecretKey>,
    pub mlkem_pk: Option<mlkem::PublicKey>,
}

impl Keyset {
    fn from_payload_v3(p: &WalletSecretPayloadV3) -> Result<Self> {
        let spend_sk = Ed25519Secret::from_bytes(&p.ed25519_spend_sk);
        let spend_pk = Ed25519Public::from(&spend_sk);
        let scan_sk = X25519Secret::from(p.x25519_scan_sk);
        let scan_pk = X25519Public::from(&scan_sk);

        let (falcon_sk, falcon_pk) = match (&p.falcon_sk_bytes, &p.falcon_pk_bytes) {
            (Some(sk), Some(pk)) => (
                Some(falcon512::SecretKey::from_bytes(sk).map_err(|_| anyhow!("Falcon SK invalid"))?),
                Some(falcon512::PublicKey::from_bytes(pk).map_err(|_| anyhow!("Falcon PK invalid"))?),
            ),
            _ => (None, None),
        };
        let (mlkem_sk, mlkem_pk) = match (&p.mlkem_sk_bytes, &p.mlkem_pk_bytes) {
            (Some(sk), Some(pk)) => (
                Some(mlkem::SecretKey::from_bytes(sk).map_err(|_| anyhow!("ML-KEM SK invalid"))?),
                Some(mlkem::PublicKey::from_bytes(pk).map_err(|_| anyhow!("ML-KEM PK invalid"))?),
            ),
            _ => (None, None),
        };

        Ok(Self { spend_sk, spend_pk, scan_sk, scan_pk, falcon_sk, falcon_pk, mlkem_sk, mlkem_pk })
    }
}

// bech32 address
fn bech32_addr(scan_pk: &X25519Public, spend_pk: &Ed25519Public) -> Result<String> {
    use bech32::{Bech32m, Hrp};
    let mut payload = Vec::with_capacity(65);
    payload.push(0x01);
    payload.extend_from_slice(scan_pk.as_bytes());
    payload.extend_from_slice(spend_pk.as_bytes());
    let hrp = Hrp::parse(BECH32_HRP)?;
    Ok(bech32::encode::<Bech32m>(hrp, &payload)?)
}

fn bech32_addr_quantum_short(scan_pk: &X25519Public, spend_pk: &Ed25519Public, falcon_pk: &falcon512::PublicKey, mlkem_pk: &mlkem::PublicKey) -> Result<String> {
    use bech32::{Bech32m, Hrp};
    let mut h = Shake256::default();
    h.update(scan_pk.as_bytes());
    h.update(spend_pk.as_bytes());
    h.update(falcon_pk.as_bytes());
    h.update(mlkem_pk.as_bytes());
    let mut rdr = h.finalize_xof();
    let mut d = [0u8;32]; rdr.read(&mut d);
    let mut payload = Vec::with_capacity(33);
    payload.push(0x03);
    payload.extend_from_slice(&d);
    let hrp = Hrp::parse("ttq")?;
    Ok(bech32::encode::<Bech32m>(hrp, &payload)?)
}

// Pepper provider
trait PepperProvider {
    fn get(&self, wallet_id: &[u8;16]) -> Result<Zeroizing<Vec<u8>>>;
}

struct NoPepper;
impl PepperProvider for NoPepper {
    fn get(&self, _id: &[u8;16]) -> Result<Zeroizing<Vec<u8>>> { Ok(Zeroizing::new(Vec::new())) }
}

struct OsLocalPepper;
impl OsLocalPepper {
    fn path_for(id: &[u8;16]) -> Result<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
            Ok(base.join("TT").join("pepper").join(hex::encode(id)))
        }
        #[cfg(not(target_os = "windows"))]
        {
            let base = config_dir().unwrap_or_else(|| PathBuf::from("."));
            Ok(base.join("tt").join("pepper").join(hex::encode(id)))
        }
    }
}

impl PepperProvider for OsLocalPepper {
    fn get(&self, wallet_id: &[u8;16]) -> Result<Zeroizing<Vec<u8>>> {
        let path = Self::path_for(wallet_id)?;
        if let Some(dir) = path.parent() { fs::create_dir_all(dir)?; }
        if path.exists() {
            let v = fs::read(&path)?;
            ensure!(v.len() == 32, "pepper file size invalid");
            return Ok(Zeroizing::new(v));
        }
        let mut p = [0u8;32]; OsRng.fill_bytes(&mut p);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut opts = OpenOptions::new(); opts.create_new(true).write(true).mode(0o600);
            let mut f = opts.open(&path)?;
            f.write_all(&p)?;
            f.sync_all()?;
        }
        #[cfg(not(unix))]
        {
            let mut opts = OpenOptions::new(); opts.create_new(true).write(true);
            let mut f = opts.open(&path)?;
            f.write_all(&p)?;
            f.sync_all()?;
        }
        Ok(Zeroizing::new(p.to_vec()))
    }
}

fn pepper_provider(pol: &PepperPolicy) -> Box<dyn PepperProvider> {
    match pol {
        PepperPolicy::None => Box::new(NoPepper),
        PepperPolicy::OsLocal => Box::new(OsLocalPepper),
    }
}

// KDF / AEAD / padding
fn derive_kdf_key(password: &str, hdr: &KdfHeader, pepper: &[u8]) -> [u8; 32] {
    match &hdr.kind {
        KdfKind::Kmac256V1 { salt32 } => {
            let k1 = ck::kmac256_derive_key(password.as_bytes(), b"TT-KDF.v5.kmac.pre", salt32);
            ck::kmac256_derive_key(&k1, b"TT-KDF.v5.kmac.post", pepper)
        }
        KdfKind::Argon2idV1 { mem_kib, time_cost, lanes, salt32 } => {
            let params = Params::new(*mem_kib, *time_cost, *lanes, Some(32)).expect("argon2 params");
            let a2 = Argon2::new_with_secret(pepper, Algorithm::Argon2id, Version::V0x13, params).expect("argon2 new_with_secret");
            let mut out = [0u8; 32];
            a2.hash_password_into(password.as_bytes(), salt32, &mut out).expect("argon2");
            ck::kmac256_derive_key(&out, b"TT-KDF.v5.post", salt32)
        }
    }
}

fn aad_for_header(h: &WalletHeader) -> Vec<u8> {
    bincode::options().with_limit(WALLET_MAX_SIZE as u64).serialize(h).expect("serialize header AAD")
}

fn pad(mut v: Vec<u8>, block: usize) -> Vec<u8> {
    let len = v.len();
    let pad_len = (block - ((len + 8) % block)) % block;
    v.extend(std::iter::repeat(0u8).take(pad_len));
    v.extend_from_slice(&(len as u64).to_le_bytes());
    v
}

fn unpad(mut v: Vec<u8>) -> Result<Vec<u8>> {
    ensure!(v.len() >= 8, "bad padded len");
    let len = u64::from_le_bytes(v[v.len()-8..].try_into().unwrap()) as usize;
    ensure!(len <= v.len()-8, "bad pad marker");
    v.truncate(len);
    Ok(v)
}

fn encrypt_wallet<T: Serialize>(payload: &T, password: &str, hdr: &WalletHeader) -> Result<Vec<u8>> {
    let prov = pepper_provider(&hdr.pepper);
    let pepper = prov.get(&hdr.wallet_id)?;
    let key = Zeroizing::new(derive_kdf_key(password, &hdr.kdf, &pepper));
    let aad = aad_for_header(hdr);
    let pt_ser = bincode::options().with_limit(WALLET_MAX_SIZE as u64).serialize(payload)?;
    let pt_pad = Zeroizing::new(pad(pt_ser, hdr.padding_block as usize));

    match hdr.aead {
        AeadKind::AesGcmSiv => {
            use aes_gcm_siv::aead::{Aead, KeyInit};
            let cipher = Aes256GcmSiv::new_from_slice(&*key).map_err(|_| anyhow!("bad AES-256 key"))?;
            let nonce = Nonce12Siv::from_slice(&hdr.nonce12);
            Ok(cipher.encrypt(nonce, aes_gcm_siv::aead::Payload { msg: pt_pad.as_ref(), aad: &aad }).map_err(|e| anyhow!("encrypt: {e}"))?)
        }
        AeadKind::XChaCha20 => {
            use chacha20poly1305::aead::{Aead, KeyInit};
            let n24 = hdr.nonce24_opt.ok_or_else(|| anyhow!("missing 24B nonce"))?;
            let cipher = XChaCha20Poly1305::new_from_slice(&*key).map_err(|_| anyhow!("bad XChaCha key"))?;
            let nonce = Nonce24::from_slice(&n24);
            Ok(cipher.encrypt(nonce, chacha20poly1305::aead::Payload { msg: pt_pad.as_ref(), aad: &aad }).map_err(|e| anyhow!("encrypt: {e}"))?)
        }
    }
}

fn decrypt_wallet_v3(enc: &[u8], password: &str, hdr: &WalletHeader) -> Result<WalletSecretPayloadV3> {
    let prov = pepper_provider(&hdr.pepper);
    let pepper = prov.get(&hdr.wallet_id)?;
    let key = Zeroizing::new(derive_kdf_key(password, &hdr.kdf, &pepper));
    let aad = aad_for_header(hdr);

    let pt = match hdr.aead {
        AeadKind::AesGcmSiv => {
            use aes_gcm_siv::aead::{Aead, KeyInit};
            let cipher = Aes256GcmSiv::new_from_slice(&*key).map_err(|_| anyhow!("bad AES-256 key"))?;
            let nonce = Nonce12Siv::from_slice(&hdr.nonce12);
            Zeroizing::new(cipher.decrypt(nonce, aes_gcm_siv::aead::Payload { msg: enc, aad: &aad }).map_err(|e| anyhow!("decrypt: {e}"))?)
        }
        AeadKind::XChaCha20 => {
            use chacha20poly1305::aead::{Aead, KeyInit};
            let n24 = hdr.nonce24_opt.ok_or_else(|| anyhow!("missing 24B nonce"))?;
            let cipher = XChaCha20Poly1305::new_from_slice(&*key).map_err(|_| anyhow!("bad XChaCha key"))?;
            let nonce = Nonce24::from_slice(&n24);
            Zeroizing::new(cipher.decrypt(nonce, chacha20poly1305::aead::Payload { msg: enc, aad: &aad }).map_err(|e| anyhow!("decrypt: {e}"))?)
        }
    };

    let unpadded = unpad(pt.to_vec())?;
    let w: WalletSecretPayloadV3 = bincode::options().with_limit(WALLET_MAX_SIZE as u64).deserialize(&unpadded)?;
    Ok(w)
}

