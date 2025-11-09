# 🔑 Falcon Deterministic Key Generation Notes

**Context:** The current `pqcrypto-falcon` library (v0.3) does **not** expose a seeded key generation API.

---

## ⚠️ Current Status

### In `FalconKeyManager::derive_epoch_keypair`:

```rust
fn derive_epoch_keypair(seed32: &[u8; 32], epoch: u64) -> (FalconSecretKey, FalconPublicKey) {
    let mut input = Vec::with_capacity(40);
    input.extend_from_slice(seed32);
    input.extend_from_slice(&epoch.to_le_bytes());
    
    // Derive deterministic seed for this epoch
    let seed = kmac256_derive_key(&input, b"FALCON_EPOCH_SEED", b"keygen");
    
    // ⚠️ TODO: pqcrypto_falcon::falcon512::keypair() does NOT accept seed!
    // Currently uses OS randomness instead of derived seed
    let (sk, pk) = falcon512::keypair();
    
    (sk, pk)
}
```

**Issue:** Epoch keys are **not deterministic** with current implementation.

---

## ✅ Solutions

### Option 1: Custom DRBG Hook (Recommended for Production)

Implement deterministic random number generator using KMAC256:

```rust
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

fn derive_epoch_keypair_deterministic(
    seed32: &[u8; 32],
    epoch: u64,
) -> (FalconSecretKey, FalconPublicKey) {
    let mut input = Vec::with_capacity(40);
    input.extend_from_slice(seed32);
    input.extend_from_slice(&epoch.to_le_bytes());
    
    // Derive 32-byte seed for ChaCha20 DRBG
    let drbg_seed = kmac256_derive_key(&input, b"FALCON_EPOCH_SEED", b"drbg");
    
    // Create deterministic RNG
    let mut rng = ChaCha20Rng::from_seed(drbg_seed);
    
    // Generate Falcon keypair using custom RNG
    // NOTE: Requires patching pqcrypto-falcon to accept RNG parameter
    // OR using a lower-level Falcon implementation (e.g., falcon-rust)
    let (sk, pk) = falcon512::keypair_from_rng(&mut rng);
    
    (sk, pk)
}
```

**Dependencies:**
```toml
[dependencies]
rand_chacha = "0.3"
rand_core = "0.6"
```

**Status:** Requires either:
- Forking `pqcrypto-falcon` to expose RNG parameter
- Using alternative Falcon library with RNG support
- Patching upstream `pqcrypto-falcon`

---

### Option 2: Store Encrypted Keys Per Epoch (Pragmatic)

Instead of deriving keys, generate once and store securely:

```rust
struct EpochKeyStore {
    /// Encrypted Falcon keys per epoch: epoch → AEAD(sk || pk)
    keys: HashMap<u64, Vec<u8>>,
    master_key: [u8; 32],
}

impl EpochKeyStore {
    fn get_or_create_epoch_key(
        &mut self,
        epoch: u64,
    ) -> Result<(FalconSecretKey, FalconPublicKey), Error> {
        if let Some(encrypted) = self.keys.get(&epoch) {
            // Decrypt existing key
            return self.decrypt_epoch_key(encrypted);
        }
        
        // Generate new keypair (using OS randomness)
        let (sk, pk) = falcon512::keypair();
        
        // Encrypt and store
        let encrypted = self.encrypt_epoch_key(&sk, &pk)?;
        self.keys.insert(epoch, encrypted);
        
        Ok((sk, pk))
    }
    
    fn encrypt_epoch_key(
        &self,
        sk: &FalconSecretKey,
        pk: &FalconPublicKey,
    ) -> Result<Vec<u8>, Error> {
        // Derive AEAD key for this epoch
        let aead_key = kmac256_derive_key(
            &self.master_key,
            b"EPOCH_KEY_ENCRYPT",
            b"",
        );
        
        // Serialize and encrypt with XChaCha20-Poly1305
        let plaintext = bincode::serialize(&(sk.as_bytes(), pk.as_bytes()))?;
        let ciphertext = aead_encrypt(&aead_key, b"", &plaintext)?;
        
        Ok(ciphertext)
    }
}
```

**Advantages:**
- ✅ Works with current `pqcrypto-falcon` (no patching needed)
- ✅ Keys are deterministic per wallet (via master key)
- ✅ Secure storage (AEAD encrypted)
- ✅ Simple to implement

**Disadvantages:**
- ❌ Requires persistent storage
- ❌ Not fully deterministic from seed alone (needs keystore file)

---

### Option 3: Use Alternative Falcon Library

Consider `falcon-rust` or similar libraries that expose seeded keygen:

```rust
// Example with hypothetical falcon-rust
use falcon_rust::{Falcon512, Params};

fn derive_epoch_keypair(seed32: &[u8; 32], epoch: u64) -> (SecretKey, PublicKey) {
    let mut input = Vec::with_capacity(40);
    input.extend_from_slice(seed32);
    input.extend_from_slice(&epoch.to_le_bytes());
    
    let drbg_seed = kmac256_derive_key(&input, b"FALCON_EPOCH_SEED", b"drbg");
    
    // Falcon-rust supports deterministic keygen
    let falcon = Falcon512::new();
    let (sk, pk) = falcon.keygen_from_seed(&drbg_seed)?;
    
    (sk, pk)
}
```

**Status:** Requires evaluation of alternative Falcon implementations.

---

## 🎯 Recommendation

### For Current Release:

**Use Option 2 (Encrypted Key Store):**
- Fast to implement
- Works with existing dependencies
- Secure and auditable

### For Future Enhancement:

**Migrate to Option 1 (Custom DRBG):**
- Fully deterministic
- Better aligns with HD wallet paradigm
- Requires upstream library support or fork

---

## 📝 Current Workaround

The current implementation generates **fresh random keys per epoch**. This is:
- ✅ **Secure** (uses OS randomness)
- ✅ **Functional** (keys work correctly)
- ❌ **Not deterministic** (cannot recover keys from seed alone)

**Impact:**
- Keys must be backed up separately
- Cannot derive historical epoch keys from master seed

**Mitigation:**
- Store encrypted keys in wallet file
- Backup wallet file regularly
- Document requirement in README

---

## 📚 References

- **pqcrypto-falcon:** https://github.com/rustpq/pqcrypto
- **Falcon Spec:** https://falcon-sign.info/
- **NIST PQC:** https://csrc.nist.gov/projects/post-quantum-cryptography

---

## ✅ Action Items

- [ ] Implement Option 2 (Encrypted Key Store) for v0.2.0
- [ ] Add README note about key backup requirement
- [ ] Evaluate alternative Falcon libraries for v0.3.0
- [ ] Consider upstream PR to `pqcrypto-falcon` for RNG parameter

---

**Status:** ⚠️ **Documented, workaround in place, enhancement planned**
