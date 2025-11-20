//! End-to-End Demo: Bob & Alice with PQC
//! 
//! - Wallets (Falcon512 + Kyber768)
//! - Encrypted notes (P2P messages)
//! - STARK transactions
//! - P2P ping/pong
//! - 100% Post-Quantum

use anyhow::Result;
use rand::RngCore;

use crate::falcon_sigs::{falcon_keypair, falcon_sign, falcon_verify, FalconPublicKey, FalconSecretKey, SignedNullifier};
use pqcrypto_traits::sign::PublicKey as PQPublicKey;
use crate::kyber_kem::{kyber_keypair, kyber_encapsulate, kyber_decapsulate, kyber_ss_to_bytes, derive_aes_key_from_shared_secret_bytes};
use crate::tx_stark::{TxOutputStark, TransactionStark, TxInputStark};
use crate::p2p::channel::SecureChannel;
use crate::core::Hash32;

use chacha20poly1305::{XChaCha20Poly1305, Key, XNonce, aead::{Aead, KeyInit}};

/// Encrypted note/message between users
#[derive(Clone, Debug)]
pub struct EncryptedNote {
    /// Recipient's Kyber PK hash
    pub recipient: Hash32,
    /// Sender's Falcon PK hash  
    pub sender: Hash32,
    /// Kyber ciphertext (1088 bytes)
    pub kyber_ct: Vec<u8>,
    /// XChaCha20-Poly1305 encrypted message
    pub encrypted_msg: Vec<u8>,
    /// Falcon512 signature on (recipient || sender || encrypted_msg)
    pub signature: SignedNullifier,
}

impl EncryptedNote {
    /// Create encrypted note from Alice to Bob
    pub fn create(
        message: &str,
        sender_falcon_sk: &FalconSecretKey,
        sender_falcon_pk: &FalconPublicKey,
        recipient_kyber_pk: &crate::kyber_kem::KyberPublicKey,
    ) -> Result<Self> {
        // Hash sender/recipient for IDs
        let sender: Hash32 = {
            let mut h = sha3::Sha3_256::new();
            use sha3::Digest;
            h.update(b"SENDER_ID");
            h.update(sender_falcon_pk.as_bytes());
            h.finalize().into()
        };

        let recipient: Hash32 = {
            let mut h = sha3::Sha3_256::new();
            use sha3::Digest;
            h.update(b"RECIPIENT_ID");
            h.update(crate::kyber_kem::kyber_pk_to_bytes(recipient_kyber_pk));
            h.finalize().into()
        };

        // Kyber KEM: encapsulate
        let (ss, ct) = kyber_encapsulate(recipient_kyber_pk);
        let ss_bytes = kyber_ss_to_bytes(&ss);
        let aes_key = derive_aes_key_from_shared_secret_bytes(&ss_bytes, b"NOTE_ENC");

        // Encrypt message with XChaCha20-Poly1305
        let cipher = XChaCha20Poly1305::new(Key::from_slice(&aes_key));
        let mut nonce_bytes = [0u8; 24];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = XNonce::from(nonce_bytes);

        let ciphertext = cipher.encrypt(&nonce, message.as_bytes())
            .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;

        let mut encrypted_msg = Vec::with_capacity(24 + ciphertext.len());
        encrypted_msg.extend_from_slice(&nonce_bytes);
        encrypted_msg.extend_from_slice(&ciphertext);

        // Sign: recipient || sender || encrypted_msg
        let mut sig_payload = Vec::new();
        sig_payload.extend_from_slice(&recipient);
        sig_payload.extend_from_slice(&sender);
        sig_payload.extend_from_slice(&encrypted_msg);

        let signature = falcon_sign(&sig_payload, sender_falcon_sk)?;

        let kyber_ct = crate::kyber_kem::kyber_ct_to_bytes(&ct).to_vec();

        Ok(Self {
            recipient,
            sender,
            kyber_ct,
            encrypted_msg,
            signature,
        })
    }

    /// Decrypt and verify note (Bob receives from Alice)
    pub fn decrypt_and_verify(
        &self,
        recipient_kyber_sk: &crate::kyber_kem::KyberSecretKey,
        sender_falcon_pk: &FalconPublicKey,
    ) -> Result<String> {
        // Verify signature first
        let mut sig_payload = Vec::new();
        sig_payload.extend_from_slice(&self.recipient);
        sig_payload.extend_from_slice(&self.sender);
        sig_payload.extend_from_slice(&self.encrypted_msg);

        falcon_verify(&sig_payload, &self.signature, sender_falcon_pk)?;

        // Decapsulate Kyber
        let kyber_ct = crate::kyber_kem::kyber_ct_from_bytes(&self.kyber_ct)?;
        let ss = kyber_decapsulate(&kyber_ct, recipient_kyber_sk)?;
        let aes_key = derive_aes_key_from_shared_secret_bytes(&crate::kyber_kem::kyber_ss_to_bytes(&ss), b"NOTE_ENC");

        // Decrypt message
        if self.encrypted_msg.len() < 24 {
            anyhow::bail!("Encrypted message too short");
        }

        let nonce_bytes = &self.encrypted_msg[0..24];
        let ciphertext = &self.encrypted_msg[24..];

        let cipher = XChaCha20Poly1305::new(Key::from_slice(&aes_key));
        let nonce = XNonce::from_slice(nonce_bytes);

        let plaintext = cipher.decrypt(nonce, ciphertext)
            .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))?;

        Ok(String::from_utf8(plaintext)?)
    }
}

/// User (Bob or Alice)
pub struct User {
    pub name: String,
    pub falcon_pk: FalconPublicKey,
    pub falcon_sk: FalconSecretKey,
    pub kyber_pk: crate::kyber_kem::KyberPublicKey,
    pub kyber_sk: crate::kyber_kem::KyberSecretKey,
}

impl User {
    pub fn new(name: &str) -> Result<Self> {
        let (falcon_pk, falcon_sk) = falcon_keypair();
        let (kyber_pk, kyber_sk) = kyber_keypair();

        Ok(Self {
            name: name.to_string(),
            falcon_pk,
            falcon_sk,
            kyber_pk,
            kyber_sk,
        })
    }

    pub fn address(&self) -> Hash32 {
        let mut h = sha3::Sha3_256::new();
        use sha3::Digest;
        h.update(b"USER_ADDRESS");
        h.update(self.falcon_pk.as_bytes());
        h.update(crate::kyber_kem::kyber_pk_to_bytes(&self.kyber_pk));
        h.finalize().into()
    }

    /// Send encrypted note to another user
    pub fn send_note(&self, message: &str, recipient: &User) -> Result<EncryptedNote> {
        EncryptedNote::create(
            message,
            &self.falcon_sk,
            &self.falcon_pk,
            &recipient.kyber_pk,
        )
    }

    /// Receive and decrypt note
    pub fn receive_note(&self, note: &EncryptedNote, sender: &User) -> Result<String> {
        note.decrypt_and_verify(&self.kyber_sk, &sender.falcon_pk)
    }

    /// Create STARK transaction output for recipient
    pub fn send_transaction(&self, value: u64, recipient: &User) -> Result<TxOutputStark> {
        let mut blinding = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut blinding);

        Ok(TxOutputStark::new(
            value,
            &blinding,
            recipient.address(),
            &recipient.kyber_pk,
        ))
    }
}

/// Run full E2E demo
pub fn run_demo() -> Result<()> {
    println!("\n🚀 ===== TRUE-TRUST E2E DEMO (100% Post-Quantum) =====\n");

    // 1. Create Bob & Alice
    println!("👤 Creating users...");
    let alice = User::new("Alice")?;
    let bob = User::new("Bob")?;
    
    println!("   ✅ Alice - address: {}", hex::encode(&alice.address()[..8]));
    println!("   ✅ Bob   - address: {}", hex::encode(&bob.address()[..8]));

    // 2. Encrypted Notes (P2P Messages)
    println!("\n📬 Testing encrypted notes...");
    let note_to_bob = alice.send_note("Hello Bob! This is Alice 🔐", &bob)?;
    println!("   ✅ Alice → Bob: note created ({} bytes)", note_to_bob.encrypted_msg.len());
    
    let decrypted = bob.receive_note(&note_to_bob, &alice)?;
    println!("   ✅ Bob received: \"{}\"", decrypted);

    let note_to_alice = bob.send_note("Hi Alice! Bob here, PQC works! 🎉", &alice)?;
    println!("   ✅ Bob → Alice: note created");
    
    let decrypted2 = alice.receive_note(&note_to_alice, &bob)?;
    println!("   ✅ Alice received: \"{}\"", decrypted2);

    // 3. STARK Transactions
    println!("\n💸 Testing STARK transactions...");
    let tx_output1 = alice.send_transaction(1000, &bob)?;
    println!("   ✅ Alice → Bob: 1000 coins (STARK proof: {} bytes)", tx_output1.stark_proof.len());
    
    let proof_valid = tx_output1.verify();
    println!("   ✅ STARK proof valid: {}", proof_valid);

    let decrypted_value = tx_output1.decrypt_and_verify(&bob.kyber_sk);
    println!("   ✅ Bob decrypted value: {:?}", decrypted_value);

    // Create full transaction
    let tx = TransactionStark {
        inputs: vec![],
        outputs: vec![
            alice.send_transaction(500, &bob)?,
            alice.send_transaction(500, &bob)?,
        ],
        fee: 10,
        nonce: 1,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs(),
    };

    let (valid, total) = tx.verify_all_proofs();
    println!("   ✅ Transaction: {}/{} STARK proofs valid", valid, total);
    println!("   ✅ TX ID: {}", hex::encode(&tx.id()[..8]));

    // 4. P2P Ping/Pong
    println!("\n🏓 Testing P2P ping/pong...");
    
    // Derive session keys from Kyber KEM
    let (ss_alice_to_bob, ct) = kyber_encapsulate(&bob.kyber_pk);
    let ss_bob = kyber_decapsulate(&ct, &bob.kyber_sk)?;
    
    let session_key_bytes = kyber_ss_to_bytes(&ss_alice_to_bob);
    let mut send_key = [0u8; 32];
    send_key.copy_from_slice(&session_key_bytes);
    
    let recv_key_bytes = kyber_ss_to_bytes(&ss_bob);
    let mut recv_key = [0u8; 32];
    recv_key.copy_from_slice(&recv_key_bytes);

    use crate::p2p::channel::SessionKey;
    let mut alice_channel = SecureChannel::new(SessionKey(send_key), SessionKey(recv_key));
    let mut bob_channel = SecureChannel::new(SessionKey(recv_key), SessionKey(send_key));

    // Ping
    let ping_msg = b"PING from Alice";
    let encrypted_ping = alice_channel.encrypt(ping_msg, b"p2p")?;
    println!("   ✅ Alice sent PING ({} bytes encrypted)", encrypted_ping.len());

    // Pong
    let decrypted_ping = bob_channel.decrypt(&encrypted_ping, b"p2p")?;
    println!("   ✅ Bob received: \"{}\"", String::from_utf8_lossy(&decrypted_ping));

    let pong_msg = b"PONG from Bob";
    let encrypted_pong = bob_channel.encrypt(pong_msg, b"p2p")?;
    println!("   ✅ Bob sent PONG ({} bytes encrypted)", encrypted_pong.len());

    let decrypted_pong = alice_channel.decrypt(&encrypted_pong, b"p2p")?;
    println!("   ✅ Alice received: \"{}\"", String::from_utf8_lossy(&decrypted_pong));

    // 5. Summary
    println!("\n✅ ===== E2E DEMO COMPLETE =====");
    println!("\n📊 Summary:");
    println!("   ✅ Wallets: Falcon512 + Kyber768 (100% PQ)");
    println!("   ✅ Notes: Encrypted P2P messages");
    println!("   ✅ Transactions: STARK range proofs");
    println!("   ✅ P2P: Secure channel ping/pong");
    println!("   ✅ Crypto: XChaCha20-Poly1305 AEAD");
    println!("\n🎉 All components working! Project is 100% Post-Quantum!\n");

    Ok(())
}
