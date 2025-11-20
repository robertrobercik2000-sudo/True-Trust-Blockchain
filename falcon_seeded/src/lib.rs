//! Deterministic Falcon-512 Signing via KMAC-DRBG
//!
//! This crate provides deterministic (reproducible) Falcon-512 key generation
//! and signing by replacing OS randomness with a user-controlled DRBG.
//!
//! # Architecture
//!
//! - **FFI to PQClean:** Direct bindings to PQClean's Falcon-512 implementation
//! - **RNG Injection:** Thread-local callback replaces `randombytes()`
//! - **Type-safe Wrapper:** Rust-friendly API with proper error handling
//! - **Deterministic:** Same seed + personalization → same keys/signatures
//!
//! # Security Properties
//!
//! - **128-bit post-quantum security** (Falcon-512)
//! - **Deterministic coins** derived from secret seed + context
//! - **Reproducible signatures** (audit-friendly, HSM/TEE compatible)
//! - **No OS RNG dependency** (full control over entropy source)
//!
//! # Example
//!
//! ```no_run
//! use falcon_seeded::{keypair_with, sign_with, verify, FillBytes};
//! use std::sync::Arc;
//!
//! // Your DRBG implementation
//! struct MyDrbg { /* ... */ }
//! impl FillBytes for MyDrbg {
//!     fn fill(&self, out: &mut [u8]) {
//!         // Fill with deterministic randomness
//!     }
//! }
//!
//! // Generate deterministic keypair
//! let drbg = Arc::new(MyDrbg::new());
//! let (pk, sk) = keypair_with(drbg.clone()).unwrap();
//!
//! // Sign message deterministically
//! let signature = sign_with(drbg, &sk, b"message").unwrap();
//!
//! // Verify (standard Falcon verification)
//! assert!(verify(&pk, b"message", &signature));
//! ```

// NOTE: This crate contains necessary `unsafe` code for FFI to PQClean C implementation.
// All unsafe blocks are carefully reviewed and confined to FFI boundaries.
#![warn(missing_docs)]

use libc::{c_int, size_t};
use std::cell::RefCell;
use std::sync::Arc;

/// Falcon-512 public key length (bytes)
pub const PK_LEN: usize = 897;

/// Falcon-512 secret key length (bytes) - PQClean format
pub const SK_LEN: usize = 1281;

/// Falcon-512 maximum signature length (bytes)
pub const SIG_MAX_LEN: usize = 690; // ~666B typical; buffer for safety

/* ============================================================================
 * FFI Declarations
 * ========================================================================== */

extern "C" {
    fn tt_falcon512_keypair_seeded(
        pk: *mut u8,
        sk: *mut u8,
        fill: extern "C" fn(*mut u8, usize),
    ) -> c_int;

    fn tt_falcon512_sign_seeded(
        sig: *mut u8,
        siglen: *mut size_t,
        m: *const u8,
        mlen: size_t,
        sk: *const u8,
        fill: extern "C" fn(*mut u8, usize),
    ) -> c_int;

    fn tt_falcon512_verify(
        sig: *const u8,
        siglen: size_t,
        m: *const u8,
        mlen: size_t,
        pk: *const u8,
    ) -> c_int;
}

/* ============================================================================
 * RNG Bridge (Rust DRBG → C randombytes)
 * ========================================================================== */

/// Trait for providing random bytes to Falcon operations
///
/// Implement this for your DRBG (e.g., `KmacDrbg`)
pub trait FillBytes: Send + Sync {
    /// Fill buffer with deterministic random bytes
    fn fill(&self, out: &mut [u8]);
}

thread_local! {
    static TLS_SRC: RefCell<Option<Arc<dyn FillBytes>>> = RefCell::new(None);
}

extern "C" fn tls_fill_adapter(out: *mut u8, outlen: usize) {
    TLS_SRC.with(|slot| {
        if let Some(src) = &*slot.borrow() {
            // Safety: out is valid pointer from C, outlen is correct size
            unsafe {
                src.fill(std::slice::from_raw_parts_mut(out, outlen));
            }
        } else {
            // Fallback: zero-fill (will cause verification failures)
            unsafe {
                std::ptr::write_bytes(out, 0u8, outlen);
            }
        }
    });
}

fn with_src<T>(src: Arc<dyn FillBytes>, f: impl FnOnce() -> T) -> T {
    TLS_SRC.with(|slot| {
        *slot.borrow_mut() = Some(src);
        let result = f();
        *slot.borrow_mut() = None;
        result
    })
}

/* ============================================================================
 * Public API
 * ========================================================================== */

/// Generate Falcon-512 keypair with deterministic RNG
///
/// # Parameters
///
/// - `src`: DRBG implementing `FillBytes` trait
///
/// # Returns
///
/// - `Ok((pk, sk))`: Public key (897 bytes) and secret key (1281 bytes)
/// - `Err`: Keygen failed (should not happen with proper DRBG)
///
/// # Example
///
/// ```no_run
/// use falcon_seeded::{keypair_with, FillBytes};
/// use std::sync::Arc;
///
/// struct MyDrbg;
/// impl FillBytes for MyDrbg {
///     fn fill(&self, out: &mut [u8]) { /* ... */ }
/// }
///
/// let drbg = Arc::new(MyDrbg);
/// let (pk, sk) = keypair_with(drbg).unwrap();
/// ```
pub fn keypair_with(src: Arc<dyn FillBytes>) -> Result<([u8; PK_LEN], [u8; SK_LEN]), &'static str> {
    let mut pk = [0u8; PK_LEN];
    let mut sk = [0u8; SK_LEN];

    let rc = with_src(src, || unsafe {
        tt_falcon512_keypair_seeded(pk.as_mut_ptr(), sk.as_mut_ptr(), tls_fill_adapter)
    });

    if rc == 0 {
        Ok((pk, sk))
    } else {
        Err("falcon keypair generation failed")
    }
}

/// Sign message with Falcon-512 using deterministic RNG
///
/// # Parameters
///
/// - `src`: DRBG implementing `FillBytes` trait (should be seeded with context)
/// - `sk`: Falcon secret key (1281 bytes)
/// - `msg`: Message to sign
///
/// # Returns
///
/// - `Ok(signature)`: Falcon signature (~666 bytes, variable length)
/// - `Err`: Signing failed
///
/// # Security Notes
///
/// **CRITICAL:** The DRBG must be seeded with:
/// 1. Secret key PRF
/// 2. Message hash or transcript
/// 3. Unique context (epoch, nonce, etc.)
///
/// Never reuse the same DRBG state for different messages!
///
/// # Example
///
/// ```no_run
/// use falcon_seeded::{sign_with, FillBytes};
/// use std::sync::Arc;
///
/// struct MyDrbg;
/// impl FillBytes for MyDrbg {
///     fn fill(&self, out: &mut [u8]) { /* ... */ }
/// }
///
/// let sk = [0u8; 1281]; // Your secret key
/// let drbg = Arc::new(MyDrbg); // Seeded with message context!
/// let sig = sign_with(drbg, &sk, b"message").unwrap();
/// ```
pub fn sign_with(
    src: Arc<dyn FillBytes>,
    sk: &[u8; SK_LEN],
    msg: &[u8],
) -> Result<Vec<u8>, &'static str> {
    let mut sig = vec![0u8; SIG_MAX_LEN];
    let mut siglen: usize = 0;

    let rc = with_src(src, || unsafe {
        tt_falcon512_sign_seeded(
            sig.as_mut_ptr(),
            &mut siglen as *mut usize,
            msg.as_ptr(),
            msg.len(),
            sk.as_ptr(),
            tls_fill_adapter,
        )
    });

    if rc == 0 {
        sig.truncate(siglen);
        Ok(sig)
    } else {
        Err("falcon signature generation failed")
    }
}

/// Verify Falcon-512 signature (standard, non-deterministic)
///
/// # Parameters
///
/// - `pk`: Falcon public key (897 bytes)
/// - `msg`: Original message
/// - `sig`: Signature to verify
///
/// # Returns
///
/// - `true`: Signature is valid
/// - `false`: Signature is invalid or malformed
///
/// # Example
///
/// ```no_run
/// use falcon_seeded::verify;
///
/// let pk = [0u8; 897]; // Public key
/// let valid = verify(&pk, b"message", &signature);
/// ```
pub fn verify(pk: &[u8; PK_LEN], msg: &[u8], sig: &[u8]) -> bool {
    let rc = unsafe {
        tt_falcon512_verify(
            sig.as_ptr(),
            sig.len(),
            msg.as_ptr(),
            msg.len(),
            pk.as_ptr(),
        )
    };
    rc == 0
}

/* ============================================================================
 * Tests
 * ========================================================================== */

#[cfg(test)]
mod tests {
    use super::*;

    // Simple deterministic DRBG for testing
    struct TestDrbg {
        counter: std::sync::Mutex<u64>,
    }

    impl TestDrbg {
        fn new() -> Self {
            Self {
                counter: std::sync::Mutex::new(0),
            }
        }
    }

    impl FillBytes for TestDrbg {
        fn fill(&self, out: &mut [u8]) {
            let mut ctr = self.counter.lock().unwrap();
            for byte in out.iter_mut() {
                *byte = (*ctr & 0xFF) as u8;
                *ctr = ctr.wrapping_add(1);
            }
        }
    }

    #[test]
    #[ignore] // Requires PQClean sources
    fn test_keypair_generation() {
        let drbg = Arc::new(TestDrbg::new());
        let result = keypair_with(drbg);
        assert!(result.is_ok(), "Keypair generation should succeed");
        
        let (pk, sk) = result.unwrap();
        assert_eq!(pk.len(), PK_LEN);
        assert_eq!(sk.len(), SK_LEN);
    }

    #[test]
    #[ignore] // Requires PQClean sources
    fn test_sign_verify() {
        let drbg_keygen = Arc::new(TestDrbg::new());
        let (pk, sk) = keypair_with(drbg_keygen).unwrap();

        let msg = b"test message";
        let drbg_sign = Arc::new(TestDrbg::new());
        let sig = sign_with(drbg_sign, &sk, msg).unwrap();

        assert!(verify(&pk, msg, &sig), "Signature should verify");
        assert!(!verify(&pk, b"wrong message", &sig), "Wrong message should fail");
    }

    #[test]
    #[ignore] // Requires PQClean sources
    fn test_deterministic_keypair() {
        let drbg1 = Arc::new(TestDrbg::new());
        let (pk1, sk1) = keypair_with(drbg1).unwrap();

        let drbg2 = Arc::new(TestDrbg::new());
        let (pk2, sk2) = keypair_with(drbg2).unwrap();

        assert_eq!(&pk1[..], &pk2[..], "Same DRBG should produce same public key");
        assert_eq!(&sk1[..], &sk2[..], "Same DRBG should produce same secret key");
    }
}