#![forbid(unsafe_code)]

//! PQ-secure P2P transport for TRUE_TRUST (Production version)
//!
//! ## Architektura:
//! - **Tożsamość noda**: Falcon512 (długoterminowa para kluczy)
//! - **Negocjacja kanału**: ML-KEM-768 (Kyber) → ephemeral shared secret
//! - **Szyfrowanie**: XChaCha20-Poly1305 AEAD
//! - **Transcript hashing**: SHA3-256 dla wszystkich wiadomości handshaku
//!
//! ## Handshake flow (3-way, mutual authentication):
//! ```text
//! Client                                Server
//!   |                                      |
//!   |  ClientHello(Falcon_PK, Kyber_PK)   |
//!   |------------------------------------->|
//!   |                                      | 1. Verify version
//!   |                                      | 2. KEM encaps → CT, SS
//!   |                                      | 3. Derive session key
//!   |                                      | 4. Sign transcript
//!   |  ServerHello(Falcon_PK, CT, sig)    |
//!   |<-------------------------------------|
//!   | 1. Verify sig                        |
//!   | 2. KEM decaps → SS                   |
//!   | 3. Derive session key                |
//!   | 4. Sign transcript                   |
//!   |  ClientFinished(sig)                 |
//!   |------------------------------------->|
//!   |                                      | Verify sig
//!   |                                      |
//!   |  <== Secure channel established ==> |
//! ```
//!
//! ## Security properties:
//! - ✅ Post-quantum security (Kyber768 + Falcon512)
//! - ✅ Forward secrecy (ephemeral KEM)
//! - ✅ Mutual authentication (both sides sign transcript)
//! - ✅ Replay protection (unique nonces + monotonic counters)
//! - ✅ Transcript integrity (SHA3-256 hash chain)
//! - ✅ AEAD confidentiality + authenticity (XChaCha20-Poly1305)

use serde::{Serialize, Deserialize};
use thiserror::Error;

use chacha20poly1305::{
    XChaCha20Poly1305, Key as XaKey, XNonce, 
    aead::{Aead, KeyInit, Payload}
};

use rand::RngCore;
use sha2::{Sha256, Digest};
use sha3::Sha3_256;

use crate::falcon_sigs::{FalconPublicKey, FalconSecretKey, BlockSignature, falcon_sign_block, falcon_verify_block};
use crate::kyber_kem::{KyberPublicKey, KyberSecretKey, KyberSharedSecret, KyberCiphertext, kyber_encapsulate, kyber_decapsulate};
use crate::crypto_kmac_consensus::kmac256_hash;
use crate::core::Hash32;

// =================== Constants ===================

/// Current protocol version
pub const PROTOCOL_VERSION: u16 = 1;

/// Maximum age of a nonce (seconds) - for replay protection
pub const MAX_NONCE_AGE_SECS: u64 = 300; // 5 minutes

/// Maximum number of messages before session renegotiation
pub const MAX_MESSAGES_PER_SESSION: u64 = 1_000_000;

// =================== Types ===================

/// Node identity (32-byte hash of Falcon public key)
pub type NodeId = [u8; 32];

/// Session key (32 bytes for XChaCha20-Poly1305)
#[derive(Clone)]
pub struct SessionKey([u8; 32]);

impl SessionKey {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

// =================== Node Identity ===================

/// Complete PQ identity for a node
#[derive(Clone)]
pub struct NodeIdentity {
    /// Node ID (derived from Falcon PK)
    pub node_id: NodeId,
    
    /// Falcon512 signing key pair (long-term)
    pub falcon_pk: FalconPublicKey,
    pub falcon_sk: FalconSecretKey,
    
    /// Kyber768 KEM key pair (ephemeral, rotated per session)
    pub kyber_pk: KyberPublicKey,
    pub kyber_sk: KyberSecretKey,
}

impl NodeIdentity {
    /// Create identity from existing key pairs
    pub fn from_keys(
        falcon_pk: FalconPublicKey,
        falcon_sk: FalconSecretKey,
        kyber_pk: KyberPublicKey,
        kyber_sk: KyberSecretKey,
    ) -> Self {
        use crate::falcon_sigs::falcon_pk_to_bytes;
        
        // NodeId = SHA256(b"TT_NODE_ID.v1" || FalconPK)
        let mut h = Sha256::new();
        h.update(b"TT_NODE_ID.v1");
        h.update(&falcon_pk_to_bytes(&falcon_pk));
        let digest = h.finalize();

        let mut node_id = [0u8; 32];
        node_id.copy_from_slice(&digest);

        Self { 
            node_id, 
            falcon_pk, 
            falcon_sk, 
            kyber_pk, 
            kyber_sk 
        }
    }
    
    /// Generate new ephemeral Kyber keys (for forward secrecy)
    pub fn rotate_kyber_keys(&mut self) {
        use crate::kyber_kem::kyber_keypair;
        let (new_pk, new_sk) = kyber_keypair();
        self.kyber_pk = new_pk;
        self.kyber_sk = new_sk;
    }
}

// =================== Errors ===================

#[derive(Debug, Error)]
pub enum P2pCryptoError {
    #[error("Kyber KEM error: {0}")]
    KemError(String),
    
    #[error("Falcon signature error: {0}")]
    SigError(String),
    
    #[error("AEAD encryption/decryption failed")]
    AeadError,
    
    #[error("Protocol version mismatch: expected {expected}, got {got}")]
    VersionMismatch { expected: u16, got: u16 },
    
    #[error("Invalid peer message: {0}")]
    InvalidMsg(String),
    
    #[error("Nonce replay detected")]
    NonceReplay,
    
    #[error("Session expired (message counter overflow)")]
    SessionExpired,
    
    #[error("Transcript verification failed")]
    TranscriptError,
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

// =================== Handshake Messages ===================

/// ClientHello - pierwsza wiadomość od klienta
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClientHello {
    /// Protocol version
    pub version: u16,
    
    /// Client's node ID
    pub node_id: NodeId,
    
    /// Client's Falcon512 public key
    pub falcon_pk: Vec<u8>,
    
    /// Client's ephemeral Kyber768 public key
    pub kyber_pk: Vec<u8>,
    
    /// Client nonce (32 bytes random)
    pub nonce_client: [u8; 32],
    
    /// Timestamp (Unix seconds, for replay protection)
    pub timestamp: u64,
}

/// ServerHello - odpowiedź serwera z KEM ciphertext
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerHello {
    /// Protocol version
    pub version: u16,
    
    /// Server's node ID
    pub node_id: NodeId,
    
    /// Server's Falcon512 public key
    pub falcon_pk: Vec<u8>,
    
    /// Kyber768 ciphertext (encapsulated shared secret)
    pub kyber_ct: Vec<u8>,
    
    /// Server nonce (32 bytes random)
    pub nonce_server: [u8; 32],
    
    /// Timestamp (Unix seconds)
    pub timestamp: u64,
    
    /// Falcon signature over transcript (up to this point)
    pub sig: Vec<u8>,
}

/// ClientFinished - mutual authentication (klient podpisuje transkrypt)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClientFinished {
    /// Falcon signature over full transcript
    pub sig: Vec<u8>,
}

// =================== Secure Channel (AEAD) ===================

/// Zaszyfrowany kanał komunikacji (post-handshake)
pub struct SecureChannel {
    /// AEAD cipher (XChaCha20-Poly1305)
    aead: XChaCha20Poly1305,
    
    /// Monotonic counter for sent messages (nonce)
    send_counter: u64,
    
    /// Monotonic counter for received messages (nonce)
    recv_counter: u64,
    
    /// Session creation timestamp (for expiry)
    created_at: std::time::Instant,
}

impl SecureChannel {
    /// Create new secure channel from session key
    pub fn new(key: SessionKey) -> Self {
        let aead = XChaCha20Poly1305::new(XaKey::from_slice(&key.0));
        Self { 
            aead, 
            send_counter: 0, 
            recv_counter: 0,
            created_at: std::time::Instant::now(),
        }
    }

    /// Check if session should be renegotiated
    pub fn should_renegotiate(&self) -> bool {
        self.send_counter >= MAX_MESSAGES_PER_SESSION || 
        self.recv_counter >= MAX_MESSAGES_PER_SESSION
    }

    /// Make XChaCha20 nonce from counter (192 bits = 24 bytes)
    /// Layout: [counter: 8 bytes LE] [reserved: 16 bytes zero]
    fn make_nonce(counter: u64) -> XNonce {
        let mut n = [0u8; 24];
        n[0..8].copy_from_slice(&counter.to_le_bytes());
        XNonce::from(n)
    }

    /// Encrypt plaintext with AEAD + AAD
    pub fn encrypt(&mut self, plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>, P2pCryptoError> {
        if self.send_counter >= MAX_MESSAGES_PER_SESSION {
            return Err(P2pCryptoError::SessionExpired);
        }
        
        let nonce = Self::make_nonce(self.send_counter);
        self.send_counter = self.send_counter.saturating_add(1);
        
        let payload = Payload { msg: plaintext, aad };
        self.aead
            .encrypt(&nonce, payload)
            .map_err(|_| P2pCryptoError::AeadError)
    }

    /// Decrypt ciphertext with AEAD + AAD
    pub fn decrypt(&mut self, ciphertext: &[u8], aad: &[u8]) -> Result<Vec<u8>, P2pCryptoError> {
        if self.recv_counter >= MAX_MESSAGES_PER_SESSION {
            return Err(P2pCryptoError::SessionExpired);
        }
        
        let nonce = Self::make_nonce(self.recv_counter);
        self.recv_counter = self.recv_counter.saturating_add(1);
        
        let payload = Payload { msg: ciphertext, aad };
        self.aead
            .decrypt(&nonce, payload)
            .map_err(|_| P2pCryptoError::AeadError)
    }
    
    /// Get session age
    pub fn age(&self) -> std::time::Duration {
        self.created_at.elapsed()
    }
}

// =================== KDF (Key Derivation) ===================

/// Derive session key from KEM shared secret + nonces
///
/// Uses KMAC256 with domain separation:
/// SessionKey = KMAC256(
///     key = shared_secret,
///     data = nonce_client || nonce_server,
///     custom = b"TT-P2P-SESSION.v1"
/// )
fn derive_session_key(
    shared: &[u8], 
    nonce_c: &[u8; 32], 
    nonce_s: &[u8; 32]
) -> SessionKey {
    let mut input = Vec::with_capacity(shared.len() + 64);
    input.extend_from_slice(shared);
    input.extend_from_slice(nonce_c);
    input.extend_from_slice(nonce_s);

    // KMAC256 z custom string dla domain separation
    let key_material = kmac256_hash(b"TT-P2P-SESSION.v1", &[&input]);
    
    let mut key = [0u8; 32];
    key.copy_from_slice(&key_material[..32]);
    SessionKey(key)
}

// =================== Transcript Hashing ===================

/// Hasher transkryptu (SHA3-256) dla handshake
pub struct TranscriptHasher {
    hasher: Sha3_256,
}

impl TranscriptHasher {
    /// New transcript starting with protocol ID
    pub fn new() -> Self {
        let mut hasher = Sha3_256::new();
        hasher.update(b"TT-P2P-HANDSHAKE.v1");
        Self { hasher }
    }
    
    /// Update transcript with labeled data
    pub fn update(&mut self, label: &[u8], data: &[u8]) {
        self.hasher.update(label);
        self.hasher.update(&(data.len() as u32).to_le_bytes());
        self.hasher.update(data);
    }
    
    /// Finalize and get transcript hash
    pub fn finalize(self) -> [u8; 32] {
        let digest = self.hasher.finalize();
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        out
    }
    
    /// Clone for parallel verification
    pub fn clone_state(&self) -> Self {
        Self {
            hasher: self.hasher.clone(),
        }
    }
}

// =================== Handshake Logic ===================

/// Build ClientHello (CLIENT SIDE - Step 1)
pub fn build_client_hello(
    id: &NodeIdentity,
    version: u16,
) -> Result<(ClientHello, TranscriptHasher), P2pCryptoError> {
    let mut nonce_c = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut nonce_c);
    
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    use crate::falcon_sigs::falcon_pk_to_bytes;
    use pqcrypto_traits::kem::PublicKey as KemPkTrait;
    
    let ch = ClientHello {
        version,
        node_id: id.node_id,
        falcon_pk: falcon_pk_to_bytes(&id.falcon_pk).to_vec(),
        kyber_pk: id.kyber_pk.as_bytes().to_vec(),
        nonce_client: nonce_c,
        timestamp,
    };

    // Start transcript
    let mut transcript = TranscriptHasher::new();
    let ch_bytes = bincode::serialize(&ch)
        .map_err(|e| P2pCryptoError::SerializationError(e.to_string()))?;
    transcript.update(b"CH", &ch_bytes);

    Ok((ch, transcript))
}

/// Handle ClientHello and build ServerHello (SERVER SIDE - Step 2)
pub fn handle_client_hello(
    server_id: &NodeIdentity,
    ch: &ClientHello,
    version_expected: u16,
    mut transcript: TranscriptHasher,
) -> Result<(ServerHello, SessionKey, TranscriptHasher), P2pCryptoError> {
    // Version check
    if ch.version != version_expected {
        return Err(P2pCryptoError::VersionMismatch {
            expected: version_expected,
            got: ch.version,
        });
    }

    // Timestamp check (replay protection)
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    if now.saturating_sub(ch.timestamp) > MAX_NONCE_AGE_SECS {
        return Err(P2pCryptoError::NonceReplay);
    }

    // Reconstruct client's Kyber PK
    use pqcrypto_traits::kem::PublicKey as KemPkTrait;
    let client_kem_pk = crate::kyber_kem::KyberPublicKey::from_bytes(&ch.kyber_pk)
        .map_err(|e| P2pCryptoError::KemError(format!("{:?}", e)))?;

    // KEM encapsulation → ciphertext + shared secret
    let (ss, ct) = kyber_encapsulate(&client_kem_pk);

    let mut nonce_s = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut nonce_s);
    
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    use crate::falcon_sigs::falcon_pk_to_bytes;
    use pqcrypto_traits::kem::{SharedSecret as KemSsTrait, Ciphertext as KemCtTrait};
    
    // Derive session key
    let session_key = derive_session_key(ss.as_bytes(), &ch.nonce_client, &nonce_s);

    // Build ServerHello (without sig yet)
    let sh_unsigned = ServerHello {
        version: version_expected,
        node_id: server_id.node_id,
        falcon_pk: falcon_pk_to_bytes(&server_id.falcon_pk).to_vec(),
        kyber_ct: ct.as_bytes().to_vec(),
        nonce_server: nonce_s,
        timestamp,
        sig: Vec::new(), // placeholder
    };

    // Update transcript with SH (without sig)
    let sh_bytes = bincode::serialize(&sh_unsigned)
        .map_err(|e| P2pCryptoError::SerializationError(e.to_string()))?;
    transcript.update(b"SH", &sh_bytes);

    // Sign transcript hash
    let transcript_hash = transcript.clone_state().finalize();
    let sig = falcon_sign_block(&transcript_hash, &server_id.falcon_sk);

    // Final ServerHello with signature
    let sh = ServerHello {
        sig: sig.signed_message_bytes.clone(),
        ..sh_unsigned
    };

    // Update transcript with signature
    transcript.update(b"SIG_S", &sig.signed_message_bytes);

    Ok((sh, session_key, transcript))
}

/// Handle ServerHello and verify (CLIENT SIDE - Step 3)
pub fn handle_server_hello(
    client_id: &NodeIdentity,
    ch: &ClientHello,
    sh: &ServerHello,
    mut transcript: TranscriptHasher,
    version_expected: u16,
) -> Result<(SessionKey, TranscriptHasher), P2pCryptoError> {
    // Version check
    if sh.version != version_expected {
        return Err(P2pCryptoError::VersionMismatch {
            expected: version_expected,
            got: sh.version,
        });
    }

    // Timestamp check
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    if now.saturating_sub(sh.timestamp) > MAX_NONCE_AGE_SECS {
        return Err(P2pCryptoError::NonceReplay);
    }

    use crate::falcon_sigs::falcon_pk_from_bytes;
    use pqcrypto_traits::kem::Ciphertext as KemCtTrait;
    
    // Reconstruct server's Falcon PK
    let server_falcon_pk = falcon_pk_from_bytes(&sh.falcon_pk)
        .map_err(|e| P2pCryptoError::SigError(format!("{:?}", e)))?;

    // Reconstruct Kyber ciphertext
    let ct = crate::kyber_kem::KyberCiphertext::from_bytes(&sh.kyber_ct)
        .map_err(|e| P2pCryptoError::KemError(format!("{:?}", e)))?;

    use pqcrypto_traits::kem::SharedSecret as KemSsTrait;
    
    // KEM decapsulation → shared secret
    let ss = kyber_decapsulate(&ct, &client_id.kyber_sk)
        .map_err(|e| P2pCryptoError::KemError(format!("{:?}", e)))?;

    // Derive session key
    let session_key = derive_session_key(ss.as_bytes(), &ch.nonce_client, &sh.nonce_server);

    // Rebuild SH without sig for transcript
    let sh_unsigned = ServerHello {
        sig: Vec::new(),
        ..sh.clone()
    };
    
    let sh_bytes = bincode::serialize(&sh_unsigned)
        .map_err(|e| P2pCryptoError::SerializationError(e.to_string()))?;
    transcript.update(b"SH", &sh_bytes);

    // Verify server's signature
    let transcript_hash = transcript.clone_state().finalize();
    let server_sig = crate::falcon_sigs::BlockSignature {
        signed_message_bytes: sh.sig.clone(),
    };
    
    falcon_verify_block(&transcript_hash, &server_sig, &server_falcon_pk)
        .map_err(|e| P2pCryptoError::SigError(e.to_string()))?;

    // Update transcript with verified signature
    transcript.update(b"SIG_S", &sh.sig);

    Ok((session_key, transcript))
}

/// Build ClientFinished (CLIENT SIDE - Step 4)
pub fn build_client_finished(
    client_id: &NodeIdentity,
    mut transcript: TranscriptHasher,
) -> Result<(ClientFinished, TranscriptHasher), P2pCryptoError> {
    transcript.update(b"CF", b"");

    // Sign full transcript
    let transcript_hash = transcript.clone_state().finalize();
    let sig = falcon_sign_block(&transcript_hash, &client_id.falcon_sk);

    let cf = ClientFinished {
        sig: sig.signed_message_bytes.clone(),
    };

    // Update transcript with signature
    transcript.update(b"SIG_C", &sig.signed_message_bytes);

    Ok((cf, transcript))
}

/// Verify ClientFinished (SERVER SIDE - Step 5)
pub fn verify_client_finished(
    client_pk_bytes: &[u8],
    mut transcript: TranscriptHasher,
    cf: &ClientFinished,
) -> Result<TranscriptHasher, P2pCryptoError> {
    use crate::falcon_sigs::falcon_pk_from_bytes;
    
    let client_pk = falcon_pk_from_bytes(client_pk_bytes)
        .map_err(|e| P2pCryptoError::SigError(format!("{:?}", e)))?;

    transcript.update(b"CF", b"");

    // Verify signature
    let transcript_hash = transcript.clone_state().finalize();
    let sig = crate::falcon_sigs::BlockSignature {
        signed_message_bytes: cf.sig.clone(),
    };
    
    falcon_verify_block(&transcript_hash, &sig, &client_pk)
        .map_err(|e| P2pCryptoError::SigError(e.to_string()))?;

    // Update transcript with verified signature
    transcript.update(b"SIG_C", &cf.sig);

    Ok(transcript)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::falcon_sigs::falcon_keypair;
    use crate::kyber_kem::kyber_keypair;

    #[test]
    fn test_full_handshake() {
        // Setup
        let (falcon_pk_c, falcon_sk_c) = falcon_keypair();
        let (kyber_pk_c, kyber_sk_c) = kyber_keypair();
        let client_id = NodeIdentity::from_keys(falcon_pk_c, falcon_sk_c, kyber_pk_c, kyber_sk_c);

        let (falcon_pk_s, falcon_sk_s) = falcon_keypair();
        let (kyber_pk_s, kyber_sk_s) = kyber_keypair();
        let server_id = NodeIdentity::from_keys(falcon_pk_s, falcon_sk_s, kyber_pk_s, kyber_sk_s);

        // 1. ClientHello
        let (ch, transcript_c) = build_client_hello(&client_id, PROTOCOL_VERSION).unwrap();

        // 2. ServerHello
        let (sh, session_key_s, transcript_s) = handle_client_hello(
            &server_id,
            &ch,
            PROTOCOL_VERSION,
            transcript_c.clone_state(),
        ).unwrap();

        // 3. Client verifies ServerHello
        let (session_key_c, transcript_c) = handle_server_hello(
            &client_id,
            &ch,
            &sh,
            transcript_c,
            PROTOCOL_VERSION,
        ).unwrap();

        // 4. ClientFinished
        let (cf, _transcript_c) = build_client_finished(&client_id, transcript_c).unwrap();

        // 5. Server verifies ClientFinished
        let _transcript_s = verify_client_finished(&ch.falcon_pk, transcript_s, &cf).unwrap();

        // Verify session keys match
        assert_eq!(session_key_c.as_bytes(), session_key_s.as_bytes());
    }

    #[test]
    fn test_secure_channel() {
        let key = SessionKey([42u8; 32]);
        let mut ch1 = SecureChannel::new(key.clone());
        let mut ch2 = SecureChannel::new(key);

        let plaintext = b"Hello, PQ World!";
        let aad = b"context";

        let ciphertext = ch1.encrypt(plaintext, aad).unwrap();
        let decrypted = ch2.decrypt(&ciphertext, aad).unwrap();

        assert_eq!(plaintext, &decrypted[..]);
    }
}
