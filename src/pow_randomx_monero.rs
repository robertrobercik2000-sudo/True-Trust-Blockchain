//! pow_randomx_monero.rs
//!
//! Monero-compatible RandomX PoW – FFI wrapper
//! ==========================================
//!
//! Ten moduł NIE implementuje RandomX samodzielnie – zamiast tego:
//! - woła oficjalną bibliotekę RandomX w C,
//! - dzięki temu dostajesz *bit-w-bit* taki sam algorytm jak Monero,
//! - Rust jest tylko cienką, bezpieczniejszą otoczką.
//!
//! Wymagania builda (wysoki poziom):
//! - sklonuj official RandomX (github.com/tevador/RandomX),
//! - zbuduj bibliotekę (np. `librandomx.a` / `librandomx.so`),
//! - w `build.rs` dodaj linkowanie:
//!     println!("cargo:rustc-link-lib=randomx");
//!     println!("cargo:rustc-link-search=native=/ścieżka/do/lib");
//!
//! Ten moduł zakłada, że nagłówek `randomx.h` jest zgodny z upstream,
//! a wartości flag odpowiadają dokładnie tym z C.
//!
//! **UWAGA**: Ten moduł wymaga `RANDOMX_FFI=1` podczas buildu.
//! Bez tego flagi, funkcje FFI nie będą linkowane (stub implementation).

#![cfg_attr(not(feature = "randomx-ffi-enabled"), allow(dead_code))]

use std::ffi::c_void;
use std::os::raw::{c_int, c_uint, c_ulonglong};
use std::ptr::NonNull;
use std::sync::Arc;
use thiserror::Error;

// c_size_t nie jest dostępne w starszych wersjach Rust
type c_size_t = usize;

/* ====== FFI: typy z C ====== */

#[repr(C)]
pub struct randomx_cache {
    _private: [u8; 0],
}

#[repr(C)]
pub struct randomx_dataset {
    _private: [u8; 0],
}

#[repr(C)]
pub struct randomx_vm {
    _private: [u8; 0],
}

pub type randomx_flags = c_uint;

/* ====== FFI: funkcje z randomx.h ====== */

// FFI funkcje są dostępne TYLKO gdy feature "randomx-ffi-enabled" jest włączony
#[cfg(feature = "randomx-ffi-enabled")]
extern "C" {
    pub fn randomx_get_flags() -> randomx_flags;

    pub fn randomx_alloc_cache(flags: randomx_flags) -> *mut randomx_cache;
    pub fn randomx_init_cache(
        cache: *mut randomx_cache,
        key: *const c_void,
        key_size: c_size_t,
    );
    pub fn randomx_reinit_cache(
        cache: *mut randomx_cache,
        key: *const c_void,
        key_size: c_size_t,
    );
    pub fn randomx_release_cache(cache: *mut randomx_cache);

    pub fn randomx_alloc_dataset(flags: randomx_flags) -> *mut randomx_dataset;
    pub fn randomx_dataset_item_count() -> c_ulonglong;
    pub fn randomx_init_dataset(
        dataset: *mut randomx_dataset,
        cache: *mut randomx_cache,
        start_item: c_ulonglong,
        item_count: c_ulonglong,
    );
    pub fn randomx_release_dataset(dataset: *mut randomx_dataset);

    pub fn randomx_create_vm(
        flags: randomx_flags,
        cache: *mut randomx_cache,
        dataset: *mut randomx_dataset,
    ) -> *mut randomx_vm;
    pub fn randomx_destroy_vm(vm: *mut randomx_vm);

    pub fn randomx_calculate_hash(
        vm: *mut randomx_vm,
        input: *const c_void,
        input_size: c_size_t,
        output: *mut c_void,
    );
}

/* ====== Flags – muszą odpowiadać randomx.h ====== */

pub const RANDOMX_FLAG_DEFAULT: randomx_flags = 0;
pub const RANDOMX_FLAG_LARGE_PAGES: randomx_flags = 1 << 0;
pub const RANDOMX_FLAG_HARD_AES: randomx_flags = 1 << 1;
pub const RANDOMX_FLAG_FULL_MEM: randomx_flags = 1 << 2;
pub const RANDOMX_FLAG_JIT: randomx_flags = 1 << 3;
pub const RANDOMX_FLAG_SECURE: randomx_flags = 1 << 4;
pub const RANDOMX_FLAG_ARGON2_SSSE3: randomx_flags = 1 << 5;
pub const RANDOMX_FLAG_ARGON2_AVX2: randomx_flags = 1 << 6;
pub const RANDOMX_FLAG_ARGON2_AVX512F: randomx_flags = 1 << 7;

/* ====== Błędy wrappera ====== */

#[derive(Debug, Error)]
pub enum RandomxError {
    #[error("randomx_alloc_cache returned null")]
    CacheAllocFailed,
    #[error("randomx_alloc_dataset returned null")]
    DatasetAllocFailed,
    #[error("randomx_create_vm returned null")]
    VmCreateFailed,
}

/* ====== Niskopoziomowe RAII-wrappers ====== */

#[cfg(feature = "randomx-ffi-enabled")]
struct Cache {
    ptr: NonNull<randomx_cache>,
}

#[cfg(feature = "randomx-ffi-enabled")]
impl Cache {
    unsafe fn new(flags: randomx_flags, key: &[u8]) -> Result<Self, RandomxError> {
        let cache_ptr = randomx_alloc_cache(flags);
        let ptr = NonNull::new(cache_ptr).ok_or(RandomxError::CacheAllocFailed)?;
        randomx_init_cache(ptr.as_ptr(), key.as_ptr() as *const c_void, key.len() as c_size_t);
        Ok(Self { ptr })
    }

    unsafe fn reinit(&mut self, key: &[u8]) {
        randomx_reinit_cache(
            self.ptr.as_ptr(),
            key.as_ptr() as *const c_void,
            key.len() as c_size_t,
        );
    }

    fn as_mut_ptr(&self) -> *mut randomx_cache {
        self.ptr.as_ptr()
    }
}

#[cfg(feature = "randomx-ffi-enabled")]
impl Drop for Cache {
    fn drop(&mut self) {
        unsafe {
            randomx_release_cache(self.ptr.as_ptr());
        }
    }
}

#[cfg(feature = "randomx-ffi-enabled")]
struct Dataset {
    ptr: NonNull<randomx_dataset>,
}

#[cfg(feature = "randomx-ffi-enabled")]
impl Dataset {
    unsafe fn new(flags: randomx_flags, cache: &Cache) -> Result<Self, RandomxError> {
        let ds_ptr = randomx_alloc_dataset(flags);
        let ptr = NonNull::new(ds_ptr).ok_or(RandomxError::DatasetAllocFailed)?;

        let item_count = randomx_dataset_item_count();
        randomx_init_dataset(ptr.as_ptr(), cache.as_mut_ptr(), 0, item_count);

        Ok(Self { ptr })
    }

    fn as_mut_ptr(&self) -> *mut randomx_dataset {
        self.ptr.as_ptr()
    }
}

#[cfg(feature = "randomx-ffi-enabled")]
impl Drop for Dataset {
    fn drop(&mut self) {
        unsafe {
            randomx_release_dataset(self.ptr.as_ptr());
        }
    }
}

#[cfg(feature = "randomx-ffi-enabled")]
struct Vm {
    ptr: NonNull<randomx_vm>,
}

#[cfg(feature = "randomx-ffi-enabled")]
impl Vm {
    unsafe fn new(
        flags: randomx_flags,
        cache: &Cache,
        dataset: &Dataset,
    ) -> Result<Self, RandomxError> {
        let vm_ptr = randomx_create_vm(flags, cache.as_mut_ptr(), dataset.as_mut_ptr());
        let ptr = NonNull::new(vm_ptr).ok_or(RandomxError::VmCreateFailed)?;
        Ok(Self { ptr })
    }

    fn as_mut_ptr(&self) -> *mut randomx_vm {
        self.ptr.as_ptr()
    }
}

#[cfg(feature = "randomx-ffi-enabled")]
impl Drop for Vm {
    fn drop(&mut self) {
        unsafe {
            randomx_destroy_vm(self.ptr.as_ptr());
        }
    }
}

/* ====== Wysokopoziomowy wrapper: RandomXEnv ====== */

#[cfg(feature = "randomx-ffi-enabled")]
pub struct RandomXEnv {
    flags: randomx_flags,
    cache: Arc<Cache>,
    dataset: Arc<Dataset>,
    vm: Vm,
}

#[cfg(feature = "randomx-ffi-enabled")]
impl RandomXEnv {
    /// Utwórz środowisko RandomX dla danego klucza (seed).
    ///
    /// `key` – seed/epoch key. Jeśli chcesz 100% zgodności z Monero
    /// dla konkretnych bloków, musisz użyć takiego samego schematu
    /// jak oni. Dla Twojego chaina możesz zdefiniować własny.
    pub fn new(key: &[u8], secure: bool) -> Result<Self, RandomxError> {
        let mut flags = unsafe { randomx_get_flags() };

        // Wymuszamy FULL_MEM + JIT jak w typowej konfiguracji
        flags |= RANDOMX_FLAG_FULL_MEM | RANDOMX_FLAG_JIT;

        if secure {
            flags |= RANDOMX_FLAG_SECURE;
        }

        let cache = unsafe { Cache::new(flags, key)? };
        let dataset = unsafe { Dataset::new(flags, &cache)? };
        let vm = unsafe { Vm::new(flags, &cache, &dataset)? };

        Ok(Self {
            flags,
            cache: Arc::new(cache),
            dataset: Arc::new(dataset),
            vm,
        })
    }

    pub fn flags(&self) -> randomx_flags {
        self.flags
    }

    pub fn hash(&mut self, input: &[u8]) -> [u8; 32] {
        let mut out = [0u8; 32];
        unsafe {
            randomx_calculate_hash(
                self.vm.as_mut_ptr(),
                input.as_ptr() as *const c_void,
                input.len() as c_size_t,
                out.as_mut_ptr() as *mut c_void,
            );
        }
        out
    }
}

/* ====== Funkcje pomocnicze pod PoW ====== */

#[cfg(feature = "randomx-ffi-enabled")]
pub fn hash_less_than(a: &[u8; 32], b: &[u8; 32]) -> bool {
    for i in 0..32 {
        if a[i] < b[i] {
            return true;
        } else if a[i] > b[i] {
            return false;
        }
    }
    false
}

#[cfg(feature = "randomx-ffi-enabled")]
pub fn mine_once(
    env: &mut RandomXEnv,
    header_without_nonce: &[u8],
    start_nonce: u64,
    max_iters: u64,
    target: &[u8; 32],
) -> Option<(u64, [u8; 32])> {
    let mut buf = Vec::with_capacity(header_without_nonce.len() + 8);

    for nonce in start_nonce..start_nonce + max_iters {
        buf.clear();
        buf.extend_from_slice(header_without_nonce);
        buf.extend_from_slice(&nonce.to_le_bytes());

        let hash = env.hash(&buf);
        if hash_less_than(&hash, target) {
            return Some((nonce, hash));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // UWAGA: Ten test wymaga:
    // 1. Zainstalowanej biblioteki RandomX (librandomx.a / librandomx.so)
    // 2. RANDOMX_FFI=1 env var podczas build
    //
    // Aby uruchomić:
    // RANDOMX_FFI=1 cargo test pow_randomx_monero::tests::test_env_and_hash_deterministic -- --ignored
    #[test]
    #[ignore]
    fn test_env_and_hash_deterministic() {
        let key = b"example-randomx-key-epoch-0";
        let mut env = RandomXEnv::new(key, false).expect("env init");

        let data = b"test block header";
        let h1 = env.hash(data);
        let h2 = env.hash(data);

        assert_eq!(h1, h2, "RandomX hash must be deterministic for same input");
    }
}
