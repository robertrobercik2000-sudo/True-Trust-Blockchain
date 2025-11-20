//! TT Private CLI - Quantum wallet v5 (PQ-only)
//!
//! - TYLKO PQC: Falcon512 + ML-KEM (Kyber768)
//! - Brak Ed25519 / X25519 (zero ECC).
//! - AEAD: AES-GCM-SIV / XChaCha20-Poly1305
//! - KDF: Argon2id z lokalnym pepperem
//! - Shamir M-of-N secret sharing (na master32)
//!
//! Adresy:
//!   - ttq: Shake256(Falcon_PK || MLKEM_PK) → 32B → Bech32m z prefixem "ttq"

#![forbid(unsafe_code)]

use anyhow::{anyhow, bail, ensure, Result};
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
use sha3::{
    digest::{ExtendableOutput, Update, XofReader},
    Shake256,
};

// crypto / keys
use aes_gcm_siv::{Aes256GcmSiv, Nonce as Nonce12Siv};
use chacha20poly1305::{XChaCha20Poly1305, XNonce as Nonce24};
use argon2::{Algorithm, Argon2, Params, Version};
use dirs::config_dir;

// PQC
use pqcrypto_falcon::falcon512;
use pqcrypto_kyber::kyber768 as mlkem;
use pqcrypto_traits::kem::{PublicKey as PQKemPublicKey, SecretKey as PQKemSecretKey};
use pqcrypto_traits::sign::{PublicKey as PQPublicKey, SecretKey as PQSecretKey};

// Shamir
use sharks::{Sharks, Share};

// Nasze KMAC / KDF
use crate::crypto::kmac as ck;

/* =========================================================================================
 * CONSTANTS
 * ====================================================================================== */

const WALLET_VERSION: u32 = 5;
const WALLET_MAX_SIZE: u64 = 1 << 20;
const BECH32_HRP_TTQ: &str = "ttq";

/* =========================================================================================
 * CLI
 * ====================================================================================== */

#[derive(Parser, Debug)]
#[command(name = "tt_priv_cli", version, author)]
#[command(about = "TRUE_TRUST wallet CLI v5 (PQ-only: Falcon512 + Kyber768)")]
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
    WalletInit {
        #[arg(long)]
        file: PathBuf,
        #[arg(long, default_value_t = true)]
        argon2: bool,
        #[arg(long, value_enum, default_value_t = AeadFlag::GcmSiv)]
        aead: AeadFlag,
        #[arg(long, value_enum, default_value_t = PepperFlag::OsLocal)]
        pepper: PepperFlag,
        #[arg(long, default_value_t = 1024)]
        pad_block: u16,
    },
    WalletAddr {
        #[arg(long)]
        file: PathBuf,
    },
    WalletExport {
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        secret: bool,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    WalletRekey {
        #[arg(long)]
        file: PathBuf,
        #[arg(long, default_value_t = true)]
        argon2: bool,
        #[arg(long, value_enum, default_value_t = AeadFlag::GcmSiv)]
        aead: AeadFlag,
        #[arg(long, value_enum, default_value_t = PepperFlag::OsLocal)]
        pepper: PepperFlag,
        #[arg(long, default_value_t = 1024)]
        pad_block: u16,
    },
    ShardsCreate {
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        out_dir: PathBuf,
        #[arg(long)]
        m: u8,
        #[arg(long)]
        n: u8,
        #[arg(long, default_value_t = false)]
        per_share_pass: bool,
    },
    ShardsRecover {
        #[arg(long, value_delimiter = ',')]
        input: Vec<PathBuf>,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = true)]
        argon2: bool,
        #[arg(long, value_enum, default_value_t = AeadFlag::GcmSiv)]
        aead: AeadFlag,
        #[arg(long, value_enum, default_value_t = PepperFlag::OsLocal)]
        pepper: PepperFlag,
        #[arg(long, default_value_t = 1024)]
        pad_block: u16,
    },
}

/* =========================================================================================
 * WALLET TYPES (PQ-only)
 * ====================================================================================== */

#[derive(Clone, Debug, Serialize, Deserialize)]
enum AeadKind {
    AesGcmSiv,
    XChaCha20,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum PepperPolicy {
    None,
    OsLocal,
}

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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct KdfHeader {
    kind: KdfKind,
    info: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum KdfKind {
    Kmac256V1 { salt32: [u8; 32] },
    Argon2idV1 {
        mem_kib: u32,
        time_cost: u32,
        lanes: u32,
        salt32: [u8; 32],
    },
}

/// Sekretny payload portfela v5 (PQ-only)
#[derive(Clone, Serialize, Deserialize, Zeroize)]
#[zeroize(drop)]
struct WalletSecretPayloadV3 {
    /// Główny seed (m.in. dla Shamir)
    master32: [u8; 32],

    /// Falcon512 SK/PK (bytes wg pqcrypto-falcon)
    falcon_sk_bytes: Vec<u8>,
    falcon_pk_bytes: Vec<u8>,

    /// ML-KEM (Kyber768) SK/PK
    mlkem_sk_bytes: Vec<u8>,
    mlkem_pk_bytes: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct WalletFile {
    header: WalletHeader,
    enc: Vec<u8>,
}

/// Zestaw kluczy PQ (już zmaterializowany z bytes)
#[derive(Clone)]
pub struct Keyset {
    pub master32: [u8; 32],
    pub falcon_sk: falcon512::SecretKey,
    pub falcon_pk: falcon512::PublicKey,
    pub mlkem_sk: mlkem::SecretKey,
    pub mlkem_pk: mlkem::PublicKey,
}

impl Keyset {
    fn from_payload_v3(p: &WalletSecretPayloadV3) -> Result<Self> {
        let falcon_sk = falcon512::SecretKey::from_bytes(&p.falcon_sk_bytes)
            .map_err(|_| anyhow!("Falcon SK invalid"))?;
        let falcon_pk = falcon512::PublicKey::from_bytes(&p.falcon_pk_bytes)
            .map_err(|_| anyhow!("Falcon PK invalid"))?;
        let mlkem_sk = mlkem::SecretKey::from_bytes(&p.mlkem_sk_bytes)
            .map_err(|_| anyhow!("ML-KEM SK invalid"))?;
        let mlkem_pk = mlkem::PublicKey::from_bytes(&p.mlkem_pk_bytes)
            .map_err(|_| anyhow!("ML-KEM PK invalid"))?;

        Ok(Self {
            master32: p.master32,
            falcon_sk,
            falcon_pk,
            mlkem_sk,
            mlkem_pk,
        })
    }
}

/* =========================================================================================
 * BECH32 ADRES PQ (ttq)
 * ====================================================================================== */

fn bech32_addr_quantum_short(
    falcon_pk: &falcon512::PublicKey,
    mlkem_pk: &mlkem::PublicKey,
) -> Result<String> {
    use bech32::{Bech32m, Hrp};

    let mut h = Shake256::default();
    h.update(falcon_pk.as_bytes());
    h.update(mlkem_pk.as_bytes());
    let mut rdr = h.finalize_xof();
    let mut d = [0u8; 32];
    rdr.read(&mut d);

    // 0x03 = typ adresu PQ (możesz zmienić, ale trzymaj stałą)
    let mut payload = Vec::with_capacity(33);
    payload.push(0x03);
    payload.extend_from_slice(&d);

    let hrp = Hrp::parse(BECH32_HRP_TTQ)?;
    Ok(bech32::encode::<Bech32m>(hrp, &payload)?)
}

/* =========================================================================================
 * PEPPER PROVIDER
 * ====================================================================================== */

trait PepperProvider {
    fn get(&self, wallet_id: &[u8; 16]) -> Result<Zeroizing<Vec<u8>>>;
}

struct NoPepper;
impl PepperProvider for NoPepper {
    fn get(&self, _id: &[u8; 16]) -> Result<Zeroizing<Vec<u8>>> {
        Ok(Zeroizing::new(Vec::new()))
    }
}

struct OsLocalPepper;
impl OsLocalPepper {
    fn path_for(id: &[u8; 16]) -> Result<PathBuf> {
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
    fn get(&self, wallet_id: &[u8; 16]) -> Result<Zeroizing<Vec<u8>>> {
        let path = Self::path_for(wallet_id)?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        if path.exists() {
            let v = fs::read(&path)?;
            ensure!(v.len() == 32, "pepper file size invalid");
            return Ok(Zeroizing::new(v));
        }
        let mut p = [0u8; 32];
        OsRng.fill_bytes(&mut p);

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut opts = OpenOptions::new();
            opts.create_new(true).write(true).mode(0o600);
            let mut f = opts.open(&path)?;
            f.write_all(&p)?;
            f.sync_all()?;
        }
        #[cfg(not(unix))]
        {
            let mut opts = OpenOptions::new();
            opts.create_new(true).write(true);
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

/* =========================================================================================
 * KDF / AEAD / PADDING
 * ====================================================================================== */

fn derive_kdf_key(password: &str, hdr: &KdfHeader, pepper: &[u8]) -> [u8; 32] {
    match &hdr.kind {
        KdfKind::Kmac256V1 { salt32 } => {
            let k1 =
                ck::kmac256_derive_key(password.as_bytes(), b"TT-KDF.v5.kmac.pre", salt32);
            ck::kmac256_derive_key(&k1, b"TT-KDF.v5.kmac.post", pepper)
        }
        KdfKind::Argon2idV1 {
            mem_kib,
            time_cost,
            lanes,
            salt32,
        } => {
            let params =
                Params::new(*mem_kib, *time_cost, *lanes, Some(32)).expect("argon2 params");
            let a2 = Argon2::new_with_secret(
                pepper,
                Algorithm::Argon2id,
                Version::V0x13,
                params,
            )
            .expect("argon2 new_with_secret");
            let mut out = [0u8; 32];
            a2.hash_password_into(password.as_bytes(), salt32, &mut out)
                .expect("argon2");
            ck::kmac256_derive_key(&out, b"TT-KDF.v5.post", salt32)
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
    let len =
        u64::from_le_bytes(v[v.len() - 8..].try_into().unwrap()) as usize;
    ensure!(len <= v.len() - 8, "bad pad marker");
    v.truncate(len);
    Ok(v)
}

fn encrypt_wallet<T: Serialize>(
    payload: &T,
    password: &str,
    hdr: &WalletHeader,
) -> Result<Vec<u8>> {
    let prov = pepper_provider(&hdr.pepper);
    let pepper = prov.get(&hdr.wallet_id)?;
    let key = Zeroizing::new(derive_kdf_key(password, &hdr.kdf, &pepper));
    let aad = aad_for_header(hdr);
    let pt_ser = bincode::options()
        .with_limit(WALLET_MAX_SIZE as u64)
        .serialize(payload)?;
    let pt_pad = Zeroizing::new(pad(pt_ser, hdr.padding_block as usize));

    match hdr.aead {
        AeadKind::AesGcmSiv => {
            use aes_gcm_siv::aead::{Aead, KeyInit};
            let cipher = Aes256GcmSiv::new_from_slice(&*key)
                .map_err(|_| anyhow!("bad AES-256 key"))?;
            let nonce = Nonce12Siv::from_slice(&hdr.nonce12);
            Ok(cipher
                .encrypt(
                    nonce,
                    aes_gcm_siv::aead::Payload {
                        msg: pt_pad.as_ref(),
                        aad: &aad,
                    },
                )
                .map_err(|e| anyhow!("encrypt: {e}"))?)
        }
        AeadKind::XChaCha20 => {
            use chacha20poly1305::aead::{Aead, KeyInit};
            let n24 =
                hdr.nonce24_opt
                    .ok_or_else(|| anyhow!("missing 24B nonce"))?;
            let cipher = XChaCha20Poly1305::new_from_slice(&*key)
                .map_err(|_| anyhow!("bad XChaCha key"))?;
            let nonce = Nonce24::from_slice(&n24);
            Ok(cipher
                .encrypt(
                    nonce,
                    chacha20poly1305::aead::Payload {
                        msg: pt_pad.as_ref(),
                        aad: &aad,
                    },
                )
                .map_err(|e| anyhow!("encrypt: {e}"))?)
        }
    }
}

fn decrypt_wallet_v3(
    enc: &[u8],
    password: &str,
    hdr: &WalletHeader,
) -> Result<WalletSecretPayloadV3> {
    let prov = pepper_provider(&hdr.pepper);
    let pepper = prov.get(&hdr.wallet_id)?;
    let key = Zeroizing::new(derive_kdf_key(password, &hdr.kdf, &pepper));
    let aad = aad_for_header(hdr);

    let pt = match hdr.aead {
        AeadKind::AesGcmSiv => {
            use aes_gcm_siv::aead::{Aead, KeyInit};
            let cipher = Aes256GcmSiv::new_from_slice(&*key)
                .map_err(|_| anyhow!("bad AES-256 key"))?;
            let nonce = Nonce12Siv::from_slice(&hdr.nonce12);
            Zeroizing::new(
                cipher
                    .decrypt(
                        nonce,
                        aes_gcm_siv::aead::Payload {
                            msg: enc,
                            aad: &aad,
                        },
                    )
                    .map_err(|e| anyhow!("decrypt: {e}"))?,
            )
        }
        AeadKind::XChaCha20 => {
            use chacha20poly1305::aead::{Aead, KeyInit};
            let n24 =
                hdr.nonce24_opt
                    .ok_or_else(|| anyhow!("missing 24B nonce"))?;
            let cipher = XChaCha20Poly1305::new_from_slice(&*key)
                .map_err(|_| anyhow!("bad XChaCha key"))?;
            let nonce = Nonce24::from_slice(&n24);
            Zeroizing::new(
                cipher
                    .decrypt(
                        nonce,
                        chacha20poly1305::aead::Payload {
                            msg: enc,
                            aad: &aad,
                        },
                    )
                    .map_err(|e| anyhow!("decrypt: {e}"))?,
            )
        }
    };

    let unpadded = unpad(pt.to_vec())?;
    let w: WalletSecretPayloadV3 = bincode::options()
        .with_limit(WALLET_MAX_SIZE as u64)
        .deserialize(&unpadded)?;
    Ok(w)
}

/* =========================================================================================
 * WALLET FILE OPERATIONS
 * ====================================================================================== */

fn load_wallet_file(path: &PathBuf) -> Result<WalletFile> {
    let meta = fs::metadata(path)?;
    ensure!(meta.len() <= WALLET_MAX_SIZE, "wallet file too large");
    let buf = fs::read(path)?;
    let wf: WalletFile = bincode::options()
        .with_limit(WALLET_MAX_SIZE as u64)
        .deserialize(&buf)?;
    ensure!(
        wf.header.version == WALLET_VERSION,
        "wallet version unsupported (have {}, want {})",
        wf.header.version,
        WALLET_VERSION
    );
    Ok(wf)
}

fn load_keyset(path: PathBuf) -> Result<(Keyset, WalletHeader)> {
    let wf = load_wallet_file(&path)?;
    let pw = Zeroizing::new(prompt_password("Password: ")?);
    let secret = decrypt_wallet_v3(&wf.enc, pw.as_str(), &wf.header)?;
    let ks = Keyset::from_payload_v3(&secret)?;
    Ok((ks, wf.header))
}

/* =========================================================================================
 * ATOMIC FILE OPERATIONS
 * ====================================================================================== */

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;

    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        opts.mode(0o600);
    }

    let mut f = opts.open(path)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    Ok(())
}

fn atomic_replace(path: &Path, bytes: &[u8]) -> Result<()> {
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;

    let tmp = path.with_extension("tmp");
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        opts.mode(0o600);
    }

    let mut f = match opts.open(&tmp) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            fs::remove_file(&tmp)?;
            opts.open(&tmp)?
        }
        Err(e) => return Err(e.into()),
    };

    f.write_all(bytes)?;
    f.sync_all()?;
    drop(f);

    match fs::rename(&tmp, path) {
        Ok(()) => {
            fsync_parent_dir(path)?;
            Ok(())
        }
        Err(_) => {
            let _ = fs::remove_file(path);
            fs::rename(&tmp, path)?;
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
fn fsync_parent_dir(_path: &Path) -> Result<()> {
    Ok(())
}

/* =========================================================================================
 * SHAMIR SECRET SHARING (na master32)
 * ====================================================================================== */

#[derive(Clone, Serialize, Deserialize)]
struct ShardHeader {
    version: u32,
    scheme: String,
    wallet_id: [u8; 16],
    m: u8,
    n: u8,
    idx: u8,
    salt32: [u8; 32],
    info: String,
    has_pw: bool,
}

#[derive(Clone, Serialize, Deserialize)]
struct ShardFile {
    hdr: ShardHeader,
    share_ct: Vec<u8>,
    mac32: [u8; 32],
}

fn shard_mac_key(wallet_id: &[u8; 16], salt32: &[u8; 32]) -> [u8; 32] {
    ck::kmac256_derive_key(wallet_id, b"TT-SHARD.mac.key", salt32)
}

fn shard_mask(share: &[u8], pw: &str, salt32: &[u8; 32]) -> Vec<u8> {
    let mask =
        ck::kmac256_xof(pw.as_bytes(), b"TT-SHARD.mask", salt32, share.len());
    share
        .iter()
        .zip(mask.iter())
        .map(|(a, b)| a ^ b)
        .collect::<Vec<u8>>()
}

fn seal_share(
    wallet_id: [u8; 16],
    idx: u8,
    m: u8,
    n: u8,
    share: &[u8],
    salt32: [u8; 32],
    pw_opt: Option<&str>,
) -> Result<ShardFile> {
    let has_pw = pw_opt.is_some();
    let share_ct = if let Some(pw) = pw_opt {
        shard_mask(share, pw, &salt32)
    } else {
        share.to_vec()
    };

    let hdr = ShardHeader {
        version: 1,
        scheme: "shamir-gf256".to_string(),
        wallet_id,
        m,
        n,
        idx,
        salt32,
        info: "TT-SHARD.v1".into(),
        has_pw,
    };

    let hdr_bytes = bincode::serialize(&hdr)?;
    let mut mac_input = hdr_bytes.clone();
    mac_input.extend(&share_ct);
    let mac32 = ck::kmac256_tag(
        &shard_mac_key(&wallet_id, &salt32),
        b"TT-SHARD.mac",
        &mac_input,
    );

    Ok(ShardFile {
        hdr,
        share_ct,
        mac32,
    })
}

fn shards_create(
    master32: [u8; 32],
    wallet_id: [u8; 16],
    m: u8,
    n: u8,
    pw_opt: Option<String>,
) -> Result<Vec<ShardFile>> {
    ensure!(m >= 2 && n >= m && n <= 8, "m-of-n out of range");
    let sharks = Sharks(m);
    let dealer = sharks.dealer(&master32);
    let shares: Vec<Share> = dealer.take(n as usize).collect();

    let mut out = Vec::with_capacity(n as usize);
    for (i, sh) in shares.into_iter().enumerate() {
        let mut salt32 = [0u8; 32];
        OsRng.fill_bytes(&mut salt32);
        let share_bytes: Vec<u8> = Vec::from(&sh);
        let sf = seal_share(
            wallet_id,
            (i + 1) as u8,
            m,
            n,
            &share_bytes,
            salt32,
            pw_opt.as_deref(),
        )?;
        out.push(sf);
    }
    Ok(out)
}

fn shards_recover(paths: &[PathBuf]) -> Result<[u8; 32]> {
    ensure!(paths.len() >= 2, "need at least 2 shards");
    let mut shards: Vec<(ShardHeader, Vec<u8>)> = Vec::new();

    for p in paths {
        let bytes = fs::read(p)?;
        let sf: ShardFile =
            serde_json::from_slice(&bytes).or_else(|_| bincode::deserialize(&bytes))?;

        // MAC verify
        let hdr_bytes = bincode::serialize(&sf.hdr)?;
        let mut mac_input = hdr_bytes.clone();
        mac_input.extend(&sf.share_ct);
        let mac_chk = ck::kmac256_tag(
            &shard_mac_key(&sf.hdr.wallet_id, &sf.hdr.salt32),
            b"TT-SHARD.mac",
            &mac_input,
        );
        ensure!(mac_chk == sf.mac32, "shard MAC mismatch: {}", p.display());
        shards.push((sf.hdr, sf.share_ct));
    }

    // Consistency
    let (wid, m, n) = (shards[0].0.wallet_id, shards[0].0.m, shards[0].0.n);
    for (h, _) in &shards {
        ensure!(
            h.wallet_id == wid && h.m == m && h.n == n,
            "shard set mismatch"
        );
    }

    // Unmask if needed
    let mut rec: Vec<(u8, Vec<u8>)> = Vec::new();
    for (h, ct) in shards {
        let pt = if h.has_pw {
            let pw = Zeroizing::new(prompt_password(format!(
                "Password for shard #{}: ",
                h.idx
            ))?);
            shard_mask(&ct, pw.as_str(), &h.salt32)
        } else {
            ct
        };
        rec.push((h.idx, pt));
    }

    // Recover secret
    let sharks = Sharks(m);
    let shares_iter = rec
        .into_iter()
        .map(|(_, bytes)| Share::try_from(bytes.as_slice()));
    let shares_vec: Result<Vec<_>> = shares_iter
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| anyhow!("share parse error: {}", e));
    let shares_vec = shares_vec?;

    let secret = sharks
        .recover(shares_vec.iter())
        .map_err(|e| anyhow!("sharks recover: {}", e))?;

    let mut out = [0u8; 32];
    out.copy_from_slice(&secret);
    Ok(out)
}

/* =========================================================================================
 * WALLET COMMANDS (PQ-only)
 * ====================================================================================== */

fn cmd_wallet_init(
    path: PathBuf,
    use_argon2: bool,
    aead_flag: AeadFlag,
    pepper_flag: PepperFlag,
    pad_block: u16,
) -> Result<()> {
    if path.exists() {
        bail!("file exists: {}", path.display());
    }

    let pw1 = Zeroizing::new(prompt_password("New password (min 12 chars): ")?);
    ensure!(pw1.len() >= 12, "password too short");
    let pw2 = Zeroizing::new(prompt_password("Repeat password: ")?);
    ensure!(pw1.as_str() == pw2.as_str(), "password mismatch");

    let mut nonce12 = [0u8; 12];
    OsRng.fill_bytes(&mut nonce12);
    let nonce24_opt = match aead_flag {
        AeadFlag::XChaCha20 => {
            let mut n = [0u8; 24];
            OsRng.fill_bytes(&mut n);
            Some(n)
        }
        _ => None,
    };

    let kdf = if use_argon2 {
        let mut salt32 = [0u8; 32];
        OsRng.fill_bytes(&mut salt32);
        let mem_kib: u32 = 512 * 1024;
        let time_cost: u32 = 3;
        let lanes: u32 = 1;
        KdfHeader {
            kind: KdfKind::Argon2idV1 {
                mem_kib,
                time_cost,
                lanes,
                salt32,
            },
            info: format!(
                "TT-KDF.v5.argon2id.t{time_cost}.m{}MiB.l{lanes}",
                mem_kib / 1024
            ),
        }
    } else {
        let mut salt32 = [0u8; 32];
        OsRng.fill_bytes(&mut salt32);
        KdfHeader {
            kind: KdfKind::Kmac256V1 { salt32 },
            info: "TT-KDF.v5.kmac".into(),
        }
    };

    let mut wallet_id = [0u8; 16];
    OsRng.fill_bytes(&mut wallet_id);

    let hdr = WalletHeader {
        version: WALLET_VERSION,
        kdf,
        aead: match aead_flag {
            AeadFlag::GcmSiv => AeadKind::AesGcmSiv,
            AeadFlag::XChaCha20 => AeadKind::XChaCha20,
        },
        nonce12,
        nonce24_opt,
        padding_block: pad_block,
        pepper: match pepper_flag {
            PepperFlag::None => PepperPolicy::None,
            PepperFlag::OsLocal => PepperPolicy::OsLocal,
        },
        wallet_id,
    };

    // master + PQ klucze
    let mut master32 = [0u8; 32];
    OsRng.fill_bytes(&mut master32);
    let (falcon_pk, falcon_sk) = falcon512::keypair();
    let (mlkem_pk, mlkem_sk) = mlkem::keypair();

    let payload = WalletSecretPayloadV3 {
        master32,
        falcon_sk_bytes: falcon_sk.as_bytes().to_vec(),
        falcon_pk_bytes: falcon_pk.as_bytes().to_vec(),
        mlkem_sk_bytes: mlkem_sk.as_bytes().to_vec(),
        mlkem_pk_bytes: mlkem_pk.as_bytes().to_vec(),
    };

    let enc = encrypt_wallet(&payload, pw1.as_str(), &hdr)?;
    let wf = WalletFile { header: hdr, enc };
    let bytes = bincode::options()
        .with_limit(WALLET_MAX_SIZE as u64)
        .serialize(&wf)?;

    atomic_write(&path, &bytes)?;
    eprintln!(
        "✅ created PQ wallet v{} → {}",
        WALLET_VERSION,
        path.display()
    );
    Ok(())
}

fn cmd_wallet_addr(path: PathBuf) -> Result<()> {
    let (ks, _hdr) = load_keyset(path)?;
    let addr = bech32_addr_quantum_short(&ks.falcon_pk, &ks.mlkem_pk)?;
    println!("address(ttq): {}", addr);
    println!("falcon_pk: {}", hex::encode(ks.falcon_pk.as_bytes()));
    println!("mlkem_pk : {}", hex::encode(ks.mlkem_pk.as_bytes()));
    Ok(())
}

fn cmd_wallet_export(
    path: PathBuf,
    secret: bool,
    out: Option<PathBuf>,
) -> Result<()> {
    let wf = load_wallet_file(&path)?;
    let pw = Zeroizing::new(prompt_password("Password: ")?);
    let secret_payload = decrypt_wallet_v3(&wf.enc, pw.as_str(), &wf.header)?;
    let ks = Keyset::from_payload_v3(&secret_payload)?;

    if secret {
        let outp =
            out.ok_or_else(|| anyhow!("secret export requires --out <file>"))?;
        let confirm =
            Zeroizing::new(prompt_password("Type wallet password again to CONFIRM: ")?);
        let _ = decrypt_wallet_v3(&wf.enc, confirm.as_str(), &wf.header)?;

        // Minimalny, prosty JSON z master32 + PQ SK
        let txt = format!(
            "{{\"version\":{},\"master32\":\"{}\",\"falcon_sk\":\"{}\",\"mlkem_sk\":\"{}\"}}\n",
            WALLET_VERSION,
            hex::encode(secret_payload.master32),
            hex::encode(ks.falcon_sk.as_bytes()),
            hex::encode(ks.mlkem_sk.as_bytes())
        );

        atomic_write(&outp, txt.as_bytes())?;
        eprintln!("🔒 secrets written → {}", outp.display());
    } else {
        let addr = bech32_addr_quantum_short(&ks.falcon_pk, &ks.mlkem_pk)?;
        println!("address(ttq): {}", addr);
        println!("falcon_pk: {}", hex::encode(ks.falcon_pk.as_bytes()));
        println!("mlkem_pk : {}", hex::encode(ks.mlkem_pk.as_bytes()));
    }
    Ok(())
}

fn cmd_wallet_rekey(
    path: PathBuf,
    use_argon2: bool,
    aead_flag: AeadFlag,
    pepper_flag: PepperFlag,
    pad_block: u16,
) -> Result<()> {
    let wf = load_wallet_file(&path)?;
    let old_pw = Zeroizing::new(prompt_password("Old password: ")?);
    let secret = decrypt_wallet_v3(&wf.enc, old_pw.as_str(), &wf.header)?;

    let pw1 = Zeroizing::new(prompt_password("New password (min 12 chars): ")?);
    ensure!(pw1.len() >= 12, "password too short");
    let pw2 = Zeroizing::new(prompt_password("Repeat password: ")?);
    ensure!(pw1.as_str() == pw2.as_str(), "password mismatch");

    let mut nonce12 = [0u8; 12];
    OsRng.fill_bytes(&mut nonce12);
    let nonce24_opt = match aead_flag {
        AeadFlag::XChaCha20 => {
            let mut n = [0u8; 24];
            OsRng.fill_bytes(&mut n);
            Some(n)
        }
        _ => None,
    };

    let kdf = if use_argon2 {
        let mut salt32 = [0u8; 32];
        OsRng.fill_bytes(&mut salt32);
        let mem_kib: u32 = 512 * 1024;
        let time_cost: u32 = 3;
        let lanes: u32 = 1;
        KdfHeader {
            kind: KdfKind::Argon2idV1 {
                mem_kib,
                time_cost,
                lanes,
                salt32,
            },
            info: format!(
                "TT-KDF.v5.argon2id.t{time_cost}.m{}MiB.l{lanes}",
                mem_kib / 1024
            ),
        }
    } else {
        let mut salt32 = [0u8; 32];
        OsRng.fill_bytes(&mut salt32);
        KdfHeader {
            kind: KdfKind::Kmac256V1 { salt32 },
            info: "TT-KDF.v5.kmac".into(),
        }
    };

    let hdr = WalletHeader {
        version: WALLET_VERSION,
        kdf,
        aead: match aead_flag {
            AeadFlag::GcmSiv => AeadKind::AesGcmSiv,
            AeadFlag::XChaCha20 => AeadKind::XChaCha20,
        },
        nonce12,
        nonce24_opt,
        padding_block: pad_block,
        pepper: match pepper_flag {
            PepperFlag::None => PepperPolicy::None,
            PepperFlag::OsLocal => PepperPolicy::OsLocal,
        },
        wallet_id: wf.header.wallet_id,
    };

    let enc = encrypt_wallet(&secret, pw1.as_str(), &hdr)?;
    let wf2 = WalletFile { header: hdr, enc };
    let bytes = bincode::options()
        .with_limit(WALLET_MAX_SIZE as u64)
        .serialize(&wf2)?;

    atomic_replace(&path, &bytes)?;
    eprintln!("🔐 rekeyed PQ wallet → {}", path.display());
    Ok(())
}

/// Tworzy nowy, zaszyfrowany portfel z zadanego master32.
///
/// Uwaga: PQ klucze są generowane na nowo (jak przy init),
/// więc adres po recovery z shardów będzie NOWY.
fn create_encrypted_wallet_from_master(
    master32: [u8; 32],
    use_argon2: bool,
    aead_flag: AeadFlag,
    pepper_flag: PepperFlag,
    pad_block: u16,
) -> Result<(WalletHeader, Vec<u8>)> {
    let pw = Zeroizing::new(
        prompt_password("Set new wallet password (min 12 chars): ")?,
    );
    ensure!(pw.len() >= 12, "password too short");
    let pw2 = Zeroizing::new(prompt_password("Repeat password: ")?);
    ensure!(pw.as_str() == pw2.as_str(), "password mismatch");

    let mut nonce12 = [0u8; 12];
    OsRng.fill_bytes(&mut nonce12);
    let nonce24_opt = match aead_flag {
        AeadFlag::XChaCha20 => {
            let mut n = [0u8; 24];
            OsRng.fill_bytes(&mut n);
            Some(n)
        }
        _ => None,
    };

    let kdf = if use_argon2 {
        let mut salt32 = [0u8; 32];
        OsRng.fill_bytes(&mut salt32);
        let mem_kib: u32 = 512 * 1024;
        let time_cost: u32 = 3;
        let lanes: u32 = 1;
        KdfHeader {
            kind: KdfKind::Argon2idV1 {
                mem_kib,
                time_cost,
                lanes,
                salt32,
            },
            info: format!(
                "TT-KDF.v5.argon2id.t{time_cost}.m{}MiB.l{lanes}",
                mem_kib / 1024
            ),
        }
    } else {
        let mut salt32 = [0u8; 32];
        OsRng.fill_bytes(&mut salt32);
        KdfHeader {
            kind: KdfKind::Kmac256V1 { salt32 },
            info: "TT-KDF.v5.kmac".into(),
        }
    };

    let mut wallet_id = [0u8; 16];
    OsRng.fill_bytes(&mut wallet_id);

    let hdr = WalletHeader {
        version: WALLET_VERSION,
        kdf,
        aead: match aead_flag {
            AeadFlag::GcmSiv => AeadKind::AesGcmSiv,
            AeadFlag::XChaCha20 => AeadKind::XChaCha20,
        },
        nonce12,
        nonce24_opt,
        padding_block: pad_block,
        pepper: match pepper_flag {
            PepperFlag::None => PepperPolicy::None,
            PepperFlag::OsLocal => PepperPolicy::OsLocal,
        },
        wallet_id,
    };

    // Nowe PQ klucze (adres się zmieni)
    let (falcon_pk, falcon_sk) = falcon512::keypair();
    let (mlkem_pk, mlkem_sk) = mlkem::keypair();

    let payload = WalletSecretPayloadV3 {
        master32,
        falcon_sk_bytes: falcon_sk.as_bytes().to_vec(),
        falcon_pk_bytes: falcon_pk.as_bytes().to_vec(),
        mlkem_sk_bytes: mlkem_sk.as_bytes().to_vec(),
        mlkem_pk_bytes: mlkem_pk.as_bytes().to_vec(),
    };

    let enc = encrypt_wallet(&payload, pw.as_str(), &hdr)?;
    Ok((hdr, enc))
}

fn cmd_shards_create(
    file: PathBuf,
    out_dir: PathBuf,
    m: u8,
    n: u8,
    per_share_pass: bool,
) -> Result<()> {
    let wf = load_wallet_file(&file)?;
    let pw = Zeroizing::new(prompt_password("Wallet password: ")?);
    let secret = decrypt_wallet_v3(&wf.enc, pw.as_str(), &wf.header)?;

    let share_pw = if per_share_pass {
        Some(Zeroizing::new(prompt_password(
            "Password for all shards: ",
        )?))
    } else {
        None
    };

    let shards = shards_create(
        secret.master32,
        wf.header.wallet_id,
        m,
        n,
        share_pw.as_deref().map(|s| s.to_string()),
    )?;

    fs::create_dir_all(&out_dir)?;

    for (i, sf) in shards.iter().enumerate() {
        let name = format!("shard-{}-of-{}.json", i + 1, n);
        let path = out_dir.join(name);
        let bytes = serde_json::to_vec_pretty(&sf)?;
        atomic_write(&path, &bytes)?;
        eprintln!("✅ wrote shard {} → {}", i + 1, path.display());
    }

    eprintln!(
        "🔐 created {}-of-{} Shamir shards in {}",
        m,
        n,
        out_dir.display()
    );
    Ok(())
}

fn cmd_shards_recover(
    input: Vec<PathBuf>,
    out: PathBuf,
    use_argon2: bool,
    aead_flag: AeadFlag,
    pepper_flag: PepperFlag,
    pad_block: u16,
) -> Result<()> {
    eprintln!("🔍 recovering master32 from {} shards...", input.len());
    let master32 = shards_recover(&input)?;

    eprintln!("✅ master32 recovered, creating new PQ wallet...");
    let (hdr, enc) = create_encrypted_wallet_from_master(
        master32,
        use_argon2,
        aead_flag,
        pepper_flag,
        pad_block,
    )?;

    let wf = WalletFile { header: hdr, enc };
    let bytes = bincode::options()
        .with_limit(WALLET_MAX_SIZE as u64)
        .serialize(&wf)?;

    atomic_write(&out, &bytes)?;
    eprintln!("✅ recovered wallet → {}", out.display());
    Ok(())
}

/* =========================================================================================
 * MAIN ENTRY POINT
 * ====================================================================================== */

pub fn run_cli() -> Result<()> {
    let cli = Cli::parse();

    match cli.cmd {
        Cmd::WalletInit {
            file,
            argon2,
            aead,
            pepper,
            pad_block,
        } => cmd_wallet_init(file, argon2, aead, pepper, pad_block),

        Cmd::WalletAddr { file } => cmd_wallet_addr(file),

        Cmd::WalletExport { file, secret, out } => {
            cmd_wallet_export(file, secret, out)
        }

        Cmd::WalletRekey {
            file,
            argon2,
            aead,
            pepper,
            pad_block,
        } => cmd_wallet_rekey(file, argon2, aead, pepper, pad_block),

        Cmd::ShardsCreate {
            file,
            out_dir,
            m,
            n,
            per_share_pass,
        } => cmd_shards_create(file, out_dir, m, n, per_share_pass),

        Cmd::ShardsRecover {
            input,
            out,
            argon2,
            aead,
            pepper,
            pad_block,
        } => cmd_shards_recover(input, out, argon2, aead, pepper, pad_block),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pad_unpad_roundtrip() {
        let data = b"hello world".to_vec();
        let padded = pad(data.clone(), 256);
        assert!(padded.len() >= data.len() + 8);
        assert_eq!(padded.len() % 256, 0);
        let unpadded = unpad(padded).unwrap();
        assert_eq!(unpadded, data);
    }

    #[test]
    fn test_shard_mask_roundtrip() {
        let share = b"secret share data";
        let pw = "password123";
        let salt = [0x42u8; 32];
        let masked = shard_mask(share, pw, &salt);
        let unmasked = shard_mask(&masked, pw, &salt);
        assert_eq!(unmasked, share);
    }
}
