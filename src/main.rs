#![forbid(unsafe_code)]

use anyhow::{anyhow, bail, ensure, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use rand::rngs::OsRng;
use rand::RngCore;
use rpassword::prompt_password;
use serde::{Deserialize, Serialize};
use bincode::Options;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use zeroize::{Zeroize, Zeroizing};

// ===== crypto / keys =====
use aes_gcm::{aead::{Aead, KeyInit}, Aes256Gcm, Nonce as Nonce12};
use aes_gcm_siv::{Aes256GcmSiv, Nonce as Nonce12Siv};
use chacha20poly1305::{XChaCha20Poly1305, XNonce as Nonce24};
use ed25519_dalek::{SigningKey as Ed25519Secret, VerifyingKey as Ed25519Public};
use x25519_dalek::{PublicKey as X25519Public, StaticSecret as X25519Secret};
use argon2::{Algorithm, Argon2, Params, Version};
use dirs::config_dir;

// ===== host modules =====
#[cfg(feature = "zk-support")]
use pot80_zk_host::crypto_kmac as ck;
#[cfg(feature = "zk-support")]
use pot80_zk_host::{
    zk,
    keyindex::KeyIndex,
    headers::HeaderHints,
    scanner::{scan_claim_with_index, ScanHit},
};

#[cfg(not(feature = "zk-support"))]
mod crypto_kmac_fallback {
    use sha3::{Shake256, digest::{Update, ExtendableOutput, XofReader}};
    
    pub fn kmac256_derive_key(key: &[u8], label: &[u8], context: &[u8]) -> [u8; 32] {
        let mut hasher = Shake256::default();
        hasher.update(b"KMAC256-DERIVE");
        hasher.update(key);
        hasher.update(label);
        hasher.update(context);
        let mut reader = hasher.finalize_xof();
        let mut out = [0u8; 32];
        reader.read(&mut out);
        out
    }
    
    pub fn kmac256_xof(key: &[u8], label: &[u8], context: &[u8], len: usize) -> Vec<u8> {
        let mut hasher = Shake256::default();
        hasher.update(b"KMAC256-XOF");
        hasher.update(key);
        hasher.update(label);
        hasher.update(context);
        let mut reader = hasher.finalize_xof();
        let mut out = vec![0u8; len];
        reader.read(&mut out);
        out
    }
    
    pub fn kmac256_tag(key: &[u8], label: &[u8], msg: &[u8]) -> [u8; 32] {
        let mut hasher = Shake256::default();
        hasher.update(b"KMAC256-TAG");
        hasher.update(key);
        hasher.update(label);
        hasher.update(msg);
        let mut reader = hasher.finalize_xof();
        let mut out = [0u8; 32];
        reader.read(&mut out);
        out
    }
}

#[cfg(not(feature = "zk-support"))]
use crypto_kmac_fallback as ck;

// ===== Shamir =====
use sharks::{Sharks, Share};

/* =========================================================================================
 * CLI
 * ====================================================================================== */

#[derive(Parser, Debug)]
#[command(name = "tt_priv_cli", version, author, about = "TRUE_TRUST wallet CLI v4 (AEAD: GCM-SIV/XChaCha20, KDF with pepper, Shamir M-of-N shards)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(ValueEnum, Clone, Debug)]
enum AeadFlag {
    GcmSiv,
    XChaCha20,
}

#[derive(ValueEnum, Clone, Debug)]
enum PepperFlag {
    None,
    OsLocal,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Initialize a new encrypted wallet file (v4)
    WalletInit {
        #[arg(long)] file: PathBuf,
        #[arg(long, default_value_t = true)] argon2: bool,
        #[arg(long, value_enum, default_value_t = AeadFlag::GcmSiv)] aead: AeadFlag,
        #[arg(long, value_enum, default_value_t = PepperFlag::OsLocal)] pepper: PepperFlag,
        #[arg(long, default_value_t = 1024)] pad_block: u16,
    },

    /// Show public address (bech32) and base public keys
    WalletAddr { #[arg(long)] file: PathBuf },

    /// Export keys (public or secret) — secret export requires --out file
    WalletExport { #[arg(long)] file: PathBuf, #[arg(long)] secret: bool, #[arg(long)] out: Option<PathBuf> },

    /// Change wallet password (re-encrypt in place)
    WalletRekey {
        #[arg(long)] file: PathBuf,
        #[arg(long, default_value_t = true)] argon2: bool,
        #[arg(long, value_enum, default_value_t = AeadFlag::GcmSiv)] aead: AeadFlag,
        #[arg(long, value_enum, default_value_t = PepperFlag::OsLocal)] pepper: PepperFlag,
        #[arg(long, default_value_t = 1024)] pad_block: u16,
    },

    // ====== Bloom / KeyIndex scanning ======
    #[cfg(feature = "zk-support")]
    FiltersInfo { #[arg(long)] dir: PathBuf },
    #[cfg(feature = "zk-support")]
    ScanReceipt { #[arg(long)] filters: PathBuf, #[arg(long)] file: PathBuf },
    #[cfg(feature = "zk-support")]
    ScanDir { #[arg(long)] filters: PathBuf, #[arg(long)] dir: PathBuf },
    #[cfg(feature = "zk-support")]
    ScanHeader { #[arg(long)] filters: PathBuf, #[arg(long)] file: PathBuf },

    // ====== Keysearch modes ======
    #[cfg(feature = "zk-support")]
    KeysearchPairs { #[arg(long)] wallet: PathBuf, #[arg(long)] file: PathBuf },
    #[cfg(feature = "zk-support")]
    KeysearchStateless { #[arg(long)] wallet: PathBuf, #[arg(long)] file: PathBuf },
    #[cfg(feature = "zk-support")]
    KeysearchHeader { #[arg(long)] wallet: PathBuf, #[arg(long)] file: PathBuf },

    // ====== Sender tools ======
    #[cfg(feature = "zk-support")]
    BuildEncHint {
        #[arg(long)] scan_pk: String,
        #[arg(long)] c_out: String,
        #[arg(long)] r_blind_hex: Option<String>,
        #[arg(long)] net_id: Option<u32>,
        #[arg(long)] value: Option<u64>,
        #[arg(long)] mask_value: bool,
        #[arg(long)] memo_utf8: Option<String>,
        #[arg(long)] memo_hex: Option<String>,
        #[arg(long)] out: Option<PathBuf>,
    },

    // ====== Shamir M-of-N backups ======
    ShardsCreate {
        #[arg(long)] file: PathBuf,
        #[arg(long)] out_dir: PathBuf,
        #[arg(long)] m: u8,
        #[arg(long)] n: u8,
        /// if set, prompt one password used to mask ALL shards
        #[arg(long, default_value_t=false)] per_share_pass: bool,
    },
    ShardsRecover {
        /// comma-separated or repeated --in
        #[arg(long, value_delimiter=',')] input: Vec<PathBuf>,
        #[arg(long)] out: PathBuf,
        #[arg(long, default_value_t = true)] argon2: bool,
        #[arg(long, value_enum, default_value_t = AeadFlag::GcmSiv)] aead: AeadFlag,
        #[arg(long, value_enum, default_value_t = PepperFlag::OsLocal)] pepper: PepperFlag,
        #[arg(long, default_value_t = 1024)] pad_block: u16,
    },
}

/* =========================================================================================
 * Wallet format
 * ====================================================================================== */

const WALLET_VERSION: u32 = 4; // AAA format
const BECH32_HRP: &str = "tt";
const WALLET_MAX_SIZE: u64 = 1 << 20; // 1 MiB

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
    wallet_id: [u8; 16], // losowe ID portfela do powiązania peppera i shardów
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct KdfHeader {
    kind: KdfKind,
    info: String, // np. "TT-KDF.v4.argon2id..."
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum KdfKind {
    /// KMAC-based KDF
    Kmac256V1 { salt32: [u8; 32] },
    /// Argon2id
    Argon2idV1 { mem_kib: u32, time_cost: u32, lanes: u32, salt32: [u8; 32] },
}

#[derive(Clone, Serialize, Deserialize, Zeroize)]
#[zeroize(drop)]
struct WalletSecretPayloadV2 {
    master32: [u8; 32],
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WalletFile {
    header: WalletHeader,
    /// AEAD over padded(serialized WalletSecretPayloadV2); AAD = bincode(header)
    enc: Vec<u8>,
}

/* =========================================================================================
 * Keyset (z derivacji)
 * ====================================================================================== */

#[derive(Clone)]
pub struct Keyset {
    pub spend_sk: Ed25519Secret,
    pub spend_pk: Ed25519Public,
    pub scan_sk:  X25519Secret,
    pub scan_pk:  X25519Public,
}

impl Keyset {
    fn from_master(master32: &[u8; 32]) -> Self {
        let spend32 = ck::kmac256_derive_key(master32, b"TT-SPEND.v1", b"seed");
        let scan32  = ck::kmac256_derive_key(master32, b"TT-SCAN.v1",  b"seed");
        let spend_sk = Ed25519Secret::from_bytes(&spend32);
        let spend_pk = Ed25519Public::from(&spend_sk);
        let scan_sk  = X25519Secret::from(scan32);
        let scan_pk  = X25519Public::from(&scan_sk);
        Self { spend_sk, spend_pk, scan_sk, scan_pk }
    }
}

/* =========================================================================================
 * Address (bech32)
 * ====================================================================================== */

fn bech32_addr(scan_pk: &X25519Public, spend_pk: &Ed25519Public) -> Result<String> {
    use bech32::{Bech32m, Hrp};
    debug_assert!(BECH32_HRP.chars().all(|c| !c.is_ascii_uppercase()), "bech32 HRP must be lowercase");
    let mut payload = Vec::with_capacity(65);
    payload.push(0x01); // version
    payload.extend_from_slice(scan_pk.as_bytes());
    payload.extend_from_slice(spend_pk.as_bytes());
    let hrp = Hrp::parse(BECH32_HRP)?;
    Ok(bech32::encode::<Bech32m>(hrp, &payload)?)
}

/* =========================================================================================
 * Pepper provider (AAA: OsLocal)
 * ====================================================================================== */

trait PepperProvider {
    fn get(&self, wallet_id: &[u8;16]) -> Result<Zeroizing<Vec<u8>>>;
}

struct NoPepper;
impl PepperProvider for NoPepper {
    fn get(&self, _id: &[u8;16]) -> Result<Zeroizing<Vec<u8>>> {
        Ok(Zeroizing::new(Vec::new()))
    }
}

struct OsLocalPepper;
impl OsLocalPepper {
    fn path_for(id: &[u8;16]) -> Result<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            let base = std::env::var_os("APPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
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
        // write 0600 / restricted
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut opts = OpenOptions::new(); opts.create_new(true).write(true).mode(0o600);
            let mut f = opts.open(&path).with_context(|| format!("create pepper {}", path.display()))?;
            f.write_all(&p)?;
            f.sync_all()?;
        }
        #[cfg(not(unix))]
        {
            let mut opts = OpenOptions::new(); opts.create_new(true).write(true);
            let mut f = opts.open(&path).with_context(|| format!("create pepper {}", path.display()))?;
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

/* =========================================================================================
 * KDF / AEAD / padding
 * ====================================================================================== */

fn derive_kdf_key(password: &str, hdr: &KdfHeader, pepper: &[u8]) -> [u8; 32] {
    match &hdr.kind {
        KdfKind::Kmac256V1 { salt32 } => {
            let k1 = ck::kmac256_derive_key(password.as_bytes(), b"TT-KDF.v4.kmac.pre", salt32);
            ck::kmac256_derive_key(&k1, b"TT-KDF.v4.kmac.post", pepper)
        }
        KdfKind::Argon2idV1 { mem_kib, time_cost, lanes, salt32 } => {
            let params = Params::new(*mem_kib, *time_cost, *lanes, Some(32)).expect("argon2 params");
            let a2 = Argon2::new_with_secret(pepper, Algorithm::Argon2id, Version::V0x13, params)
                .expect("argon2 new_with_secret");
            let mut out = [0u8; 32];
            a2.hash_password_into(password.as_bytes(), salt32, &mut out).expect("argon2");
            ck::kmac256_derive_key(&out, b"TT-KDF.v4.post", salt32)
        }
    }
}

fn aad_for_header(h: &WalletHeader) -> Vec<u8> {
    bincode::options()
        .with_limit(WALLET_MAX_SIZE as u64)
        .serialize(h)
        .expect("serialize header AAD")
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

fn encrypt_wallet(payload: &WalletSecretPayloadV2, password: &str, hdr: &WalletHeader) -> Result<Vec<u8>> {
    let prov = pepper_provider(&hdr.pepper);
    let pepper = prov.get(&hdr.wallet_id)?;
    let key = Zeroizing::new(derive_kdf_key(password, &hdr.kdf, &pepper));
    let aad = aad_for_header(hdr);
    let pt_ser = bincode::options().with_limit(WALLET_MAX_SIZE as u64).serialize(payload)?;
    let pt_pad = Zeroizing::new(pad(pt_ser, hdr.padding_block as usize));

    match hdr.aead {
        AeadKind::AesGcmSiv => {
            let cipher = Aes256GcmSiv::new_from_slice(&*key).map_err(|_| anyhow!("bad AES-256 key"))?;
            let nonce = Nonce12Siv::from_slice(&hdr.nonce12);
            Ok(cipher.encrypt(nonce, aes_gcm_siv::aead::Payload { msg: pt_pad.as_ref(), aad: &aad })
                .map_err(|e| anyhow!("encrypt: {e}"))?)
        }
        AeadKind::XChaCha20 => {
            let n24 = hdr.nonce24_opt.ok_or_else(|| anyhow!("missing 24B nonce"))?;
            let cipher = XChaCha20Poly1305::new_from_slice(&*key).map_err(|_| anyhow!("bad XChaCha key"))?;
            let nonce = Nonce24::from_slice(&n24);
            Ok(cipher.encrypt(nonce, chacha20poly1305::aead::Payload { msg: pt_pad.as_ref(), aad: &aad })
                .map_err(|e| anyhow!("encrypt: {e}"))?)
        }
    }
}

fn decrypt_wallet(enc: &[u8], password: &str, hdr: &WalletHeader) -> Result<WalletSecretPayloadV2> {
    let prov = pepper_provider(&hdr.pepper);
    let pepper = prov.get(&hdr.wallet_id)?;
    let key = Zeroizing::new(derive_kdf_key(password, &hdr.kdf, &pepper));
    let aad = aad_for_header(hdr);

    let pt = match hdr.aead {
        AeadKind::AesGcmSiv => {
            let cipher = Aes256GcmSiv::new_from_slice(&*key).map_err(|_| anyhow!("bad AES-256 key"))?;
            let nonce = Nonce12Siv::from_slice(&hdr.nonce12);
            Zeroizing::new(cipher.decrypt(nonce, aes_gcm_siv::aead::Payload { msg: enc, aad: &aad })
                .map_err(|e| anyhow!("decrypt: {e}"))?)
        }
        AeadKind::XChaCha20 => {
            let n24 = hdr.nonce24_opt.ok_or_else(|| anyhow!("missing 24B nonce"))?;
            let cipher = XChaCha20Poly1305::new_from_slice(&*key).map_err(|_| anyhow!("bad XChaCha key"))?;
            let nonce = Nonce24::from_slice(&n24);
            Zeroizing::new(cipher.decrypt(nonce, chacha20poly1305::aead::Payload { msg: enc, aad: &aad })
                .map_err(|e| anyhow!("decrypt: {e}"))?)
        }
    };

    let unpadded = unpad(pt.to_vec())?;
    let w: WalletSecretPayloadV2 = bincode::options()
        .with_limit(WALLET_MAX_SIZE as u64)
        .deserialize(&unpadded)?;
    Ok(w)
}

/* =========================================================================================
 * Helpers (Bloom/scan)
 * ====================================================================================== */

fn load_wallet_file(path: &PathBuf) -> Result<WalletFile> {
    let meta = fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
    ensure!(meta.len() <= WALLET_MAX_SIZE, "wallet file too large");
    let buf = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let wf: WalletFile = bincode::options().with_limit(WALLET_MAX_SIZE as u64).deserialize(&buf)?;
    ensure!(wf.header.version == WALLET_VERSION, "wallet version unsupported (have {}, want {})", wf.header.version, WALLET_VERSION);
    Ok(wf)
}

fn load_keyset(path: PathBuf) -> Result<Keyset> {
    let wf = load_wallet_file(&path)?;
    let pw = Zeroizing::new(prompt_password("Password: ")?);
    let secret = decrypt_wallet(&wf.enc, pw.as_str(), &wf.header)?;
    let ks = Keyset::from_master(&secret.master32);
    Ok(ks)
}

#[cfg(feature = "zk-support")]
fn load_keyindex(dir: &Path) -> Result<KeyIndex> {
    KeyIndex::load_latest(dir).context("load latest KeyIndex")
}

#[cfg(feature = "zk-support")]
fn print_hits(hits: &[ScanHit]) {
    println!("hits: {}", hits.len());
    for (i, h) in hits.iter().enumerate() {
        println!(
            "#{i}: tag=0x{:04x} out_idx={} enc_hint32={} note_pt={}",
            h.filter_tag16,
            h.out_idx,
            hex::encode(h.enc_hint32),
            hex::encode(&h.note_commit_point),
        );
    }
}

/* =========================================================================================
 * Atomic save helpers
 * ====================================================================================== */

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;

    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    { opts.mode(0o600); }

    let mut f = opts.open(path).with_context(|| format!("create_new {}", path.display()))?;
    f.write_all(bytes)?;
    f.sync_all()?; // fsync
    Ok(())
}

/// Atomic replace: writes to path.tmp then renames over original
fn atomic_replace(path: &Path, bytes: &[u8]) -> Result<()> {
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;

    let tmp = path.with_extension("tmp");
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    { opts.mode(0o600); }

    let mut f = match opts.open(&tmp) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            fs::remove_file(&tmp)?;
            opts.open(&tmp).with_context(|| format!("create_new (after clear) {}", tmp.display()))?
        },
        Err(e) => return Err(e).with_context(|| format!("create_new {}", tmp.display())),
    };

    f.write_all(bytes)?;
    f.sync_all()?;
    drop(f);

    match fs::rename(&tmp, path) {
        Ok(()) => { fsync_parent_dir(path)?; Ok(()) }
        Err(_) => {
            let _ = fs::remove_file(path);
            fs::rename(&tmp, path).with_context(|| format!("rename fallback {} -> {}", tmp.display(), path.display()))?;
            fsync_parent_dir(path)?;
            Ok(())
        }
    }
}

#[cfg(unix)]
fn fsync_parent_dir(path: &Path) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let dirf = std::fs::File::open(parent)?;
    dirf.sync_all()?;
    Ok(())
}
#[cfg(not(unix))]
fn fsync_parent_dir(_path: &Path) -> Result<()> { Ok(()) }

/* =========================================================================================
 * Shamir shards (AAA)
 * ====================================================================================== */

#[derive(Clone, Serialize, Deserialize)]
struct ShardHeader {
    version: u32,             // 1
    scheme: String,           // "shamir-gf256"
    wallet_id: [u8;16],
    m: u8,
    n: u8,
    idx: u8,                  // 1..n
    salt32: [u8;32],          // do maskowania share/maca
    info: String,             // "TT-SHARD.v1"
    has_pw: bool,             // czy maskowano hasłem
}

#[derive(Clone, Serialize, Deserialize)]
struct ShardFile {
    hdr: ShardHeader,
    share_ct: Vec<u8>,
    mac32: [u8;32],
}

fn shard_mac_key(wallet_id: &[u8;16], salt32: &[u8;32]) -> [u8;32] {
    ck::kmac256_derive_key(wallet_id, b"TT-SHARD.mac.key", salt32)
}

fn shard_mask(share: &[u8], pw: &str, salt32: &[u8;32]) -> Vec<u8> {
    let mask = ck::kmac256_xof(pw.as_bytes(), b"TT-SHARD.mask", salt32, share.len());
    share.iter().zip(mask).map(|(a,b)| a^b).collect::<Vec<u8>>()
}

fn seal_share(wallet_id: [u8;16], idx: u8, m: u8, n: u8, share: &[u8], salt32: [u8;32], pw_opt: Option<&str>) -> Result<ShardFile> {
    let has_pw = pw_opt.is_some();
    let share_ct = if let Some(pw) = pw_opt {
        shard_mask(share, pw, &salt32)
    } else {
        share.to_vec()
    };
    let hdr = ShardHeader {
        version: 1, scheme: "shamir-gf256".to_string(), wallet_id, m, n, idx, salt32, info: "TT-SHARD.v1".into(), has_pw
    };
    let hdr_bytes = bincode::serialize(&hdr)?;
    let mut mac_input = hdr_bytes.clone();
    mac_input.extend(&share_ct);
    let mac32 = ck::kmac256_tag(&shard_mac_key(&wallet_id, &salt32), b"TT-SHARD.mac", &mac_input);
    Ok(ShardFile { hdr, share_ct, mac32 })
}

fn shards_create(master32: [u8;32], wallet_id: [u8;16], m: u8, n: u8, pw_opt: Option<String>) -> Result<Vec<ShardFile>> {
    ensure!(m>=2 && n>=m && n<=8, "m-of-n out of range");
    let sharks = Sharks(m);
    let dealer = sharks.dealer(&master32);
    let shares: Vec<Share> = dealer.take(n as usize).collect();

    let mut out = Vec::with_capacity(n as usize);
    for (i, sh) in shares.into_iter().enumerate() {
        let mut salt32=[0u8;32]; OsRng.fill_bytes(&mut salt32);
        // Convert Share to bytes
        let share_bytes: Vec<u8> = Vec::from(&sh);
        let sf = seal_share(wallet_id, (i+1) as u8, m, n, &share_bytes, salt32, pw_opt.as_deref())?;
        out.push(sf);
    }
    Ok(out)
}

fn shards_recover(paths: &[PathBuf]) -> Result<[u8;32]> {
    ensure!(paths.len()>=2, "need at least 2 shards");
    let mut shards: Vec<(ShardHeader, Vec<u8>)> = Vec::new();
    for p in paths {
        let bytes = fs::read(p)?;
        let sf: ShardFile = serde_json::from_slice(&bytes)
            .or_else(|_| bincode::deserialize(&bytes))
            .with_context(|| format!("parse shard {}", p.display()))?;
        // MAC verify
        let hdr_bytes = bincode::serialize(&sf.hdr)?;
        let mut mac_input = hdr_bytes.clone(); mac_input.extend(&sf.share_ct);
        let mac_chk = ck::kmac256_tag(&shard_mac_key(&sf.hdr.wallet_id, &sf.hdr.salt32), b"TT-SHARD.mac", &mac_input);
        ensure!(mac_chk == sf.mac32, "shard MAC mismatch: {}", p.display());
        shards.push((sf.hdr, sf.share_ct));
    }
    // Consistency
    let (wid, m, n) = (shards[0].0.wallet_id, shards[0].0.m, shards[0].0.n);
    for (h,_) in &shards { ensure!(h.wallet_id==wid && h.m==m && h.n==n, "shard set mismatch"); }

    // Possibly password unmask
    let mut rec: Vec<(u8, Vec<u8>)> = Vec::new();
    for (h, ct) in shards {
        let pt = if h.has_pw {
            let pw = Zeroizing::new(prompt_password(format!("Password for shard #{}: ", h.idx))?);
            shard_mask(&ct, pw.as_str(), &h.salt32)
        } else { ct };
        rec.push((h.idx, pt));
    }

    let sharks = Sharks(m);
    let shares_iter = rec.into_iter().map(|(i, bytes)| Share::try_from(bytes.as_slice()).map(|sh| (i, sh)));
    let shares_vec: Result<Vec<_>> = shares_iter.collect::<Result<Vec<_>, _>>()
        .map_err(|e| anyhow!("share parse error: {}", e));
    let shares_vec = shares_vec?;
    
    let secret = sharks.recover(shares_vec.iter().map(|(_, sh)| sh))
        .map_err(|e| anyhow!("sharks recover: {}", e))?;
    let mut out=[0u8;32]; out.copy_from_slice(&secret);
    Ok(out)
}

/* =========================================================================================
 * Commands impl
 * ====================================================================================== */

fn cmd_wallet_init(path: PathBuf, use_argon2: bool, aead_flag: AeadFlag, pepper_flag: PepperFlag, pad_block: u16) -> Result<()> {
    if path.exists() { bail!("file exists: {}", path.display()); }

    let pw1 = Zeroizing::new(prompt_password("New password (min 12 chars): ")?);
    ensure!(pw1.len() >= 12, "password too short");
    let pw2 = Zeroizing::new(prompt_password("Repeat password: ")?);
    ensure!(pw1.as_str() == pw2.as_str(), "password mismatch");

    let mut nonce12 = [0u8; 12]; OsRng.fill_bytes(&mut nonce12);
    let nonce24_opt = match aead_flag {
        AeadFlag::XChaCha20 => {
            let mut n=[0u8;24]; OsRng.fill_bytes(&mut n); Some(n)
        }
        _ => None
    };

    // KDF header
    let kdf = if use_argon2 {
        let mut salt32 = [0u8; 32]; OsRng.fill_bytes(&mut salt32);
        let mem_kib: u32 = 512 * 1024; // 512 MiB baseline
        let time_cost: u32 = 3;
        let lanes: u32 = 1;
        KdfHeader { kind: KdfKind::Argon2idV1 { mem_kib, time_cost, lanes, salt32 }, info: format!("TT-KDF.v4.argon2id.t{time_cost}.m{}MiB.l{lanes}", mem_kib/1024) }
    } else {
        let mut salt32 = [0u8; 32]; OsRng.fill_bytes(&mut salt32);
        KdfHeader { kind: KdfKind::Kmac256V1 { salt32 }, info: "TT-KDF.v4.kmac".into() }
    };

    let mut wallet_id=[0u8;16]; OsRng.fill_bytes(&mut wallet_id);
    let hdr = WalletHeader {
        version: WALLET_VERSION,
        kdf,
        aead: match aead_flag { AeadFlag::GcmSiv => AeadKind::AesGcmSiv, AeadFlag::XChaCha20 => AeadKind::XChaCha20 },
        nonce12,
        nonce24_opt,
        padding_block: pad_block,
        pepper: match pepper_flag { PepperFlag::None => PepperPolicy::None, PepperFlag::OsLocal => PepperPolicy::OsLocal },
        wallet_id,
    };

    // LOSOWY master
    let mut master32 = [0u8; 32]; OsRng.fill_bytes(&mut master32);
    let payload = WalletSecretPayloadV2 { master32 };

    let enc = encrypt_wallet(&payload, pw1.as_str(), &hdr)?;

    let wf = WalletFile { header: hdr, enc };
    let bytes = bincode::options().with_limit(WALLET_MAX_SIZE as u64).serialize(&wf)?;

    atomic_write(&path, &bytes)?;
    eprintln!("✅ created wallet v{}: {}", WALLET_VERSION, path.display());
    Ok(())
}

fn cmd_wallet_addr(path: PathBuf) -> Result<()> {
    let ks = load_keyset(path)?;
    let addr = bech32_addr(&ks.scan_pk, &ks.spend_pk)?;
    println!("address: {}", addr);
    println!("scan_pk (x25519): {}", hex::encode(ks.scan_pk.as_bytes()));
    println!("spend_pk(ed25519): {}", hex::encode(ks.spend_pk.to_bytes()));
    Ok(())
}

fn cmd_wallet_export(path: PathBuf, secret: bool, out: Option<PathBuf>) -> Result<()> {
    let wf = load_wallet_file(&path)?;
    let pw = Zeroizing::new(prompt_password("Password: ")?);
    let secret_payload = decrypt_wallet(&wf.enc, pw.as_str(), &wf.header)?;
    let ks = Keyset::from_master(&secret_payload.master32);
    if secret {
        let outp = out.ok_or_else(|| anyhow!("secret export requires --out <file> (STDOUT disabled)"))?;
        let confirm = Zeroizing::new(prompt_password("Type wallet password again to CONFIRM secret export: ")?);
        let _ = decrypt_wallet(&wf.enc, confirm.as_str(), &wf.header)?;
        let txt = format!(
            "{{\"master32\":\"{}\",\"scan_sk\":\"{}\",\"spend_sk\":\"{}\"}}\n",
            hex::encode(secret_payload.master32),
            hex::encode(ks.scan_sk.to_bytes()),
            hex::encode(ks.spend_sk.to_bytes()),
        );
        atomic_write(&outp, txt.as_bytes())?;
        eprintln!("🔒 secrets written → {}", outp.display());
    } else {
        println!("scan_pk: {}", hex::encode(ks.scan_pk.as_bytes()));
        println!("spend_pk: {}", hex::encode(ks.spend_pk.to_bytes()));
    }
    Ok(())
}

fn cmd_wallet_rekey(path: PathBuf, use_argon2: bool, aead_flag: AeadFlag, pepper_flag: PepperFlag, pad_block: u16) -> Result<()> {
    let wf = load_wallet_file(&path)?;
    let old_pw = Zeroizing::new(prompt_password("Old password: ")?);
    let secret = decrypt_wallet(&wf.enc, old_pw.as_str(), &wf.header)?;

    let pw1 = Zeroizing::new(prompt_password("New password: ")?);
    let pw2 = Zeroizing::new(prompt_password("Repeat password: ")?);
    ensure!(pw1.as_str() == pw2.as_str(), "password mismatch");

    let mut nonce12 = [0u8; 12]; OsRng.fill_bytes(&mut nonce12);
    let nonce24_opt = match aead_flag {
        AeadFlag::XChaCha20 => { let mut n=[0u8;24]; OsRng.fill_bytes(&mut n); Some(n) }
        _ => None
    };

    let kdf = if use_argon2 {
        let mut salt32 = [0u8; 32]; OsRng.fill_bytes(&mut salt32);
        let mem_kib: u32 = 512 * 1024;
        let time_cost: u32 = 3;
        let lanes: u32 = 1;
        KdfHeader { kind: KdfKind::Argon2idV1 { mem_kib, time_cost, lanes, salt32 }, info: format!("TT-KDF.v4.argon2id.t{time_cost}.m{}MiB.l{lanes}", mem_kib/1024) }
    } else {
        let mut salt32 = [0u8; 32]; OsRng.fill_bytes(&mut salt32);
        KdfHeader { kind: KdfKind::Kmac256V1 { salt32 }, info: "TT-KDF.v4.kmac".into() }
    };

    let hdr = WalletHeader {
        version: WALLET_VERSION,
        kdf,
        aead: match aead_flag { AeadFlag::GcmSiv => AeadKind::AesGcmSiv, AeadFlag::XChaCha20 => AeadKind::XChaCha20 },
        nonce12,
        nonce24_opt,
        padding_block: pad_block,
        pepper: match pepper_flag { PepperFlag::None => PepperPolicy::None, PepperFlag::OsLocal => PepperPolicy::OsLocal },
        wallet_id: wf.header.wallet_id, // zachowujemy ten sam wallet_id
    };

    let enc = encrypt_wallet(&secret, pw1.as_str(), &hdr)?;
    let wf2 = WalletFile { header: hdr, enc };
    let bytes = bincode::options().with_limit(WALLET_MAX_SIZE as u64).serialize(&wf2)?;
    atomic_replace(&path, &bytes)?;
    eprintln!("🔐 rekeyed wallet: {}", path.display());
    Ok(())
}

#[cfg(feature = "zk-support")]
fn cmd_filters_info(dir: PathBuf) -> Result<()> {
    let idx = load_keyindex(&dir)?;
    println!(
        "filters: epoch={} m_bits={} k_hashes={} file={}",
        idx.epoch, idx.bloom.m_bits, idx.bloom.k_hashes, idx.path.display()
    );
    Ok(())
}

#[cfg(feature = "zk-support")]
fn cmd_scan_receipt(filters: PathBuf, file: PathBuf) -> Result<()> {
    let idx = load_keyindex(&filters)?;
    let bytes = fs::read(&file).with_context(|| format!("read {}", file.display()))?;
    let claim = zk::verify_priv_receipt(&bytes)?;
    let hits = scan_claim_with_index(&claim, &idx)?;
    print_hits(&hits);
    Ok(())
}

#[cfg(feature = "zk-support")]
fn cmd_scan_dir(filters: PathBuf, dir: PathBuf) -> Result<()> {
    let idx = load_keyindex(&filters)?;
    let mut total_hits = 0usize;
    for entry in fs::read_dir(&dir).with_context(|| format!("list {}", dir.display()))? {
        let p = entry?.path();
        if !p.is_file() { continue; }
        let Ok(bytes) = fs::read(&p) else { continue };
        if let Ok(claim) = zk::verify_priv_receipt(&bytes) {
            if let Ok(hits) = scan_claim_with_index(&claim, &idx) {
                if !hits.is_empty() {
                    println!("file: {}", p.display());
                    print_hits(&hits);
                    total_hits += hits.len();
                }
            }
        }
    }
    println!("total hits: {}", total_hits);
    Ok(())
}

#[cfg(feature = "zk-support")]
fn cmd_scan_header(filters: PathBuf, file: PathBuf) -> Result<()> {
    let idx = load_keyindex(&filters)?;
    let packed = fs::read(&file).with_context(|| format!("read {}", file.display()))?;
    let hh = HeaderHints::unpack(&packed).context("header hints unpack failed")?;
    let mut cnt = 0usize;
    for e in &hh.entries {
        if idx.bloom.contains(&e.filter_tag16) { cnt += 1; }
    }
    println!("header-hits: {} / {} entries", cnt, hh.entries.len());
    Ok(())
}

/* =========================================================================================
 * Keysearch helpers & commands
 * ====================================================================================== */

#[cfg(feature = "zk-support")]
#[derive(Deserialize)]
struct PairFull { c_out: String, enc_hint: String }

#[cfg(feature = "zk-support")]
#[derive(Deserialize)]
struct PairStateless { c_out: String, eph_pub: String, enc_hint32: String }

#[cfg(feature = "zk-support")]
#[derive(Deserialize)]
struct PairHeader { eph_pub: String, hdr_tag16: String }

#[cfg(feature = "zk-support")]
fn hex32(s: &str) -> Result<[u8;32]> {
    let v = hex::decode(s.trim())?;
    ensure!(v.len()==32, "hex32 length");
    let mut a=[0u8;32]; a.copy_from_slice(&v); Ok(a)
}

#[cfg(feature = "zk-support")]
fn hex16(s: &str) -> Result<[u8;16]> {
    let v = hex::decode(s.trim())?;
    ensure!(v.len()==16, "hex16 length");
    let mut a=[0u8;16]; a.copy_from_slice(&v); Ok(a)
}

#[cfg(feature = "zk-support")]
fn hexv(s: &str) -> Result<Vec<u8>> { Ok(hex::decode(s.trim())?) }

#[cfg(feature = "zk-support")]
fn cmd_keysearch_pairs(wallet: PathBuf, file: PathBuf) -> Result<()> {
    use pot80_zk_host::keysearch::{KeySearchCtx, AadMode, MAX_ENC_HINT_BYTES};

    let ks = load_keyset(wallet)?;
    let view = ks.scan_sk.to_bytes();
    let ctx = KeySearchCtx::new(view);

    let txt = fs::read_to_string(&file).with_context(|| format!("read {}", file.display()))?;
    let mut hits = 0usize; let mut total = 0usize;

    for (lineno, line) in txt.lines().enumerate() {
        if line.trim().is_empty() { continue; }
        let rec: PairFull = serde_json::from_str(line)
            .with_context(|| format!("jsonl line {}", lineno+1))?;

        let c_out = hex32(&rec.c_out)?;
        let enc   = hexv(&rec.enc_hint)?;
        total += 1;

        if enc.len() > MAX_ENC_HINT_BYTES {
            eprintln!("skip line {}: enc_hint too large ({} bytes)", lineno+1, enc.len());
            continue;
        }

        if let Some((k, maybe)) = ctx.try_match_and_decrypt_ext(&c_out, &enc, AadMode::COutOnly) {
            match maybe {
                Some(dec) => {
                    println!(
                        "hit #{}: c_out={} k_search={} value={:?} tlv_items={} r_blind={}",
                        total,
                        rec.c_out,
                        hex::encode(k),
                        dec.value,
                        dec.memo_items.len(),
                        hex::encode(dec.r_blind)
                    );
                }
                None => {
                    println!(
                        "hit #{}: c_out={} k_search={} (no payload)",
                        total, rec.c_out, hex::encode(k)
                    );
                }
            }
            hits += 1;
        }
    }

    println!("total: {} hits: {}", total, hits);
    Ok(())
}

#[cfg(feature = "zk-support")]
fn cmd_keysearch_stateless(wallet: PathBuf, file: PathBuf) -> Result<()> {
    let ks = load_keyset(wallet)?; let view = ks.scan_sk.to_bytes();
    let ctx = pot80_zk_host::keysearch::KeySearchCtx::new(view);

    let txt = fs::read_to_string(&file).with_context(|| format!("read {}", file.display()))?;
    let mut hits = 0usize; let mut total = 0usize;
    for (lineno, line) in txt.lines().enumerate() {
        if line.trim().is_empty() { continue; }
        let rec: PairStateless = serde_json::from_str(line).with_context(|| format!("jsonl line {}", lineno+1))?;
        let c_out = hex32(&rec.c_out)?; let eph = hex32(&rec.eph_pub)?; let h = hex32(&rec.enc_hint32)?; total += 1;
        if let Some(k) = ctx.try_match_stateless(&c_out, &eph, &h) {
            println!("hit #{}: c_out={} k_search={}", total, rec.c_out, hex::encode(k));
            hits += 1;
        }
    }
    println!("total: {} hits: {}", total, hits);
    Ok(())
}

#[cfg(feature = "zk-support")]
fn cmd_keysearch_header(wallet: PathBuf, file: PathBuf) -> Result<()> {
    let ks = load_keyset(wallet)?; let view = ks.scan_sk.to_bytes();
    let ctx = pot80_zk_host::keysearch::KeySearchCtx::new(view);

    let txt = fs::read_to_string(&file).with_context(|| format!("read {}", file.display()))?;
    let mut hits = 0usize; let mut total = 0usize;
    for (lineno, line) in txt.lines().enumerate() {
        if line.trim().is_empty() { continue; }
        let rec: PairHeader = serde_json::from_str(line).with_context(|| format!("jsonl line {}", lineno+1))?;
        let eph = hex32(&rec.eph_pub)?; let t16 = hex16(&rec.hdr_tag16)?; total += 1;
        if ctx.header_hit(&eph, &t16) {
            println!("prefilter hit #{}: eph_pub={}", total, rec.eph_pub);
            hits += 1;
        }
    }
    println!("total: {} prefilter hits: {}", total, hits);
    Ok(())
}

#[cfg(feature = "zk-support")]
fn cmd_build_enc_hint(
    scan_pk_hex: String,
    c_out_hex: String,
    r_blind_hex: Option<String>,
    net_id: Option<u32>,
    value: Option<u64>,
    mask_value: bool,
    memo_utf8: Option<String>,
    memo_hex: Option<String>,
    out: Option<PathBuf>,
) -> Result<()> {
    use pot80_zk_host::keysearch::{KeySearchCtx, AadMode, ValueConceal, tlv};

    let scan_pk_arr = hex32(&scan_pk_hex)?;
    let c_out = hex32(&c_out_hex)?;
    let scan_pk = X25519Public::from(scan_pk_arr);

    let r_blind = if let Some(h) = r_blind_hex { hex32(&h)? } else { let mut r=[0u8;32]; OsRng.fill_bytes(&mut r); r };

    let aad_mode = if let Some(n) = net_id { AadMode::NetIdAndCOut(n) } else { AadMode::COutOnly };

    let val_mode = match (value, mask_value) {
        (Some(v), true)  => ValueConceal::Masked(v),
        (Some(v), false) => ValueConceal::Plain(v),
        (None,     _ )   => ValueConceal::None,
    };

    let mut items: Vec<tlv::Item> = Vec::new();
    if let Some(s) = memo_utf8 { items.push(tlv::Item::Ascii(s)); }
    if let Some(h) = memo_hex { items.push(tlv::Item::CiphertextToSpend(hex::decode(h.trim())?)); }

    let enc = KeySearchCtx::build_enc_hint_ext(&scan_pk, &c_out, aad_mode, Some(r_blind), val_mode, &items);

    if let Some(path) = out {
        fs::write(&path, &enc)?;
        eprintln!("✅ wrote enc_hint: {} bytes → {}", enc.len(), path.display());
    } else {
        println!("{}", hex::encode(enc));
    }
    Ok(())
}

/* =========================================================================================
 * Shards Commands
 * ====================================================================================== */

fn cmd_shards_create(file: PathBuf, out_dir: PathBuf, m:u8, n:u8, per_share_pass: bool) -> Result<()> {
    let wf = load_wallet_file(&file)?;
    let pw = Zeroizing::new(prompt_password("Password: ")?);
    let secret = decrypt_wallet(&wf.enc, pw.as_str(), &wf.header)?;
    let pw_opt = if per_share_pass {
        let p = prompt_password("Password for ALL shards (enter to skip): ")?;
        if p.is_empty() { None } else { Some(p) }
    } else { None };
    let shards = shards_create(secret.master32, wf.header.wallet_id, m, n, pw_opt)?;
    fs::create_dir_all(&out_dir)?;
    for s in shards {
        let name = format!("ttshard-{:02}-of-{:02}-id{}.json", s.hdr.idx, s.hdr.n, hex::encode(&s.hdr.wallet_id));
        let p = out_dir.join(name);
        let bytes = serde_json::to_vec_pretty(&s)?;
        atomic_write(&p, &bytes)?;
        eprintln!("🧩 wrote {}", p.display());
    }
    Ok(())
}

fn cmd_shards_recover(input: Vec<PathBuf>, out: PathBuf, use_argon2: bool, aead_flag: AeadFlag, pepper_flag: PepperFlag, pad_block: u16) -> Result<()> {
    let master32 = shards_recover(&input)?;
    let (hdr, enc) = create_encrypted_wallet_from_master(master32, use_argon2, aead_flag, pepper_flag, pad_block)?;
    let wf = WalletFile { header: hdr, enc };
    let bytes = bincode::options().with_limit(WALLET_MAX_SIZE as u64).serialize(&wf)?;
    atomic_write(&out, &bytes)?;
    eprintln!("✅ recovered new wallet: {}", out.display());
    Ok(())
}

/* =========================================================================================
 * Helpers to construct wallet from master
 * ====================================================================================== */

fn create_encrypted_wallet_from_master(master32: [u8;32], use_argon2: bool, aead_flag: AeadFlag, pepper_flag: PepperFlag, pad_block: u16) -> Result<(WalletHeader, Vec<u8>)> {
    let pw = Zeroizing::new(prompt_password("Set new wallet password (min 12 chars): ")?);
    ensure!(pw.len() >= 12, "password too short");
    let pw2 = Zeroizing::new(prompt_password("Repeat password: ")?);
    ensure!(pw.as_str() == pw2.as_str(), "password mismatch");

    let mut nonce12 = [0u8; 12]; OsRng.fill_bytes(&mut nonce12);
    let nonce24_opt = match aead_flag {
        AeadFlag::XChaCha20 => { let mut n=[0u8;24]; OsRng.fill_bytes(&mut n); Some(n) }
        _ => None
    };

    let kdf = if use_argon2 {
        let mut salt32 = [0u8; 32]; OsRng.fill_bytes(&mut salt32);
        let mem_kib: u32 = 512 * 1024;
        let time_cost: u32 = 3;
        let lanes: u32 = 1;
        KdfHeader { kind: KdfKind::Argon2idV1 { mem_kib, time_cost, lanes, salt32 }, info: format!("TT-KDF.v4.argon2id.t{time_cost}.m{}MiB.l{lanes}", mem_kib/1024) }
    } else {
        let mut salt32 = [0u8; 32]; OsRng.fill_bytes(&mut salt32);
        KdfHeader { kind: KdfKind::Kmac256V1 { salt32 }, info: "TT-KDF.v4.kmac".into() }
    };

    let mut wallet_id=[0u8;16]; OsRng.fill_bytes(&mut wallet_id);
    let hdr = WalletHeader {
        version: WALLET_VERSION,
        kdf,
        aead: match aead_flag { AeadFlag::GcmSiv => AeadKind::AesGcmSiv, AeadFlag::XChaCha20 => AeadKind::XChaCha20 },
        nonce12,
        nonce24_opt,
        padding_block: pad_block,
        pepper: match pepper_flag { PepperFlag::None => PepperPolicy::None, PepperFlag::OsLocal => PepperPolicy::OsLocal },
        wallet_id,
    };

    let payload = WalletSecretPayloadV2 { master32 };
    let enc = encrypt_wallet(&payload, pw.as_str(), &hdr)?;
    Ok((hdr, enc))
}

/* =========================================================================================
 * main
 * ====================================================================================== */

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::WalletInit { file, argon2, aead, pepper, pad_block } =>
            cmd_wallet_init(file, argon2, aead, pepper, pad_block)?,

        Cmd::WalletAddr { file } => cmd_wallet_addr(file)?,

        Cmd::WalletExport { file, secret, out } => cmd_wallet_export(file, secret, out)?,

        Cmd::WalletRekey { file, argon2, aead, pepper, pad_block } =>
            cmd_wallet_rekey(file, argon2, aead, pepper, pad_block)?,

        #[cfg(feature = "zk-support")]
        Cmd::FiltersInfo { dir } => cmd_filters_info(dir)?,
        #[cfg(feature = "zk-support")]
        Cmd::ScanReceipt { filters, file } => cmd_scan_receipt(filters, file)?,
        #[cfg(feature = "zk-support")]
        Cmd::ScanDir { filters, dir } => cmd_scan_dir(filters, dir)?,
        #[cfg(feature = "zk-support")]
        Cmd::ScanHeader { filters, file } => cmd_scan_header(filters, file)?,

        #[cfg(feature = "zk-support")]
        Cmd::KeysearchPairs { wallet, file } => cmd_keysearch_pairs(wallet, file)?,
        #[cfg(feature = "zk-support")]
        Cmd::KeysearchStateless { wallet, file } => cmd_keysearch_stateless(wallet, file)?,
        #[cfg(feature = "zk-support")]
        Cmd::KeysearchHeader { wallet, file } => cmd_keysearch_header(wallet, file)?,

        #[cfg(feature = "zk-support")]
        Cmd::BuildEncHint { scan_pk, c_out, r_blind_hex, net_id, value, mask_value, memo_utf8, memo_hex, out } =>
            cmd_build_enc_hint(scan_pk, c_out, r_blind_hex, net_id, value, mask_value, memo_utf8, memo_hex, out)?,

        Cmd::ShardsCreate { file, out_dir, m, n, per_share_pass } =>
            cmd_shards_create(file, out_dir, m, n, per_share_pass)?,

        Cmd::ShardsRecover { input, out, argon2, aead, pepper, pad_block } =>
            cmd_shards_recover(input, out, argon2, aead, pepper, pad_block)?,
    }
    Ok(())
}
