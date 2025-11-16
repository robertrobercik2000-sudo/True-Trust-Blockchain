//! Full RandomX binding (PRO version, Monero-compatible via C FFI)
//!
//! Ten moduł nie implementuje RandomX w czystym Ruście – zamiast tego
//! używa oficjalnej biblioteki RandomX (C) przez FFI.
//!
//! Publiczne API zostaje takie samo jak wcześniej:
//! - `RandomXHasher::new(epoch: u64)`
//! - `fn hash(&self, input: &[u8]) -> [u8; 32]`
//! - `fn verify(&self, input: &[u8], expected: &[u8; 32]) -> bool`
//! - `fn mine_randomx(...) -> Option<(u64, [u8; 32])>`
//!
//! Dzięki temu inne moduły (golden_trio, cpu_mining, itp.)
//! nie wymagają zmian – dostają tylko „prawdziwy" RandomX pod spodem.

use std::ffi::c_void;
use std::os::raw::{c_uint, c_ulonglong};
use std::ptr::NonNull;
use std::sync::Mutex;
use std::time::Instant;

use thiserror::Error;

// c_size_t compatibility
type c_size_t = usize;

/* ======== FFI: typy i funkcje z randomx.h ======== */

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

/* ======== Flags (tak jak w randomx.h) ======== */

pub const RANDOMX_FLAG_DEFAULT: randomx_flags = 0;
pub const RANDOMX_FLAG_LARGE_PAGES: randomx_flags = 1 << 0;
pub const RANDOMX_FLAG_HARD_AES: randomx_flags = 1 << 1;
pub const RANDOMX_FLAG_FULL_MEM: randomx_flags = 1 << 2;
pub const RANDOMX_FLAG_JIT: randomx_flags = 1 << 3;
pub const RANDOMX_FLAG_SECURE: randomx_flags = 1 << 4;
pub const RANDOMX_FLAG_ARGON2_SSSE3: randomx_flags = 1 << 5;
pub const RANDOMX_FLAG_ARGON2_AVX2: randomx_flags = 1 << 6;
pub const RANDOMX_FLAG_ARGON2_AVX512F: randomx_flags = 1 << 7;

/* ======== Błędy inicjalizacji ======== */

#[derive(Debug, Error)]
pub enum RandomxError {
    #[error("randomx_alloc_cache returned null")]
    CacheAllocFailed,
    #[error("randomx_alloc_dataset returned null")]
    DatasetAllocFailed,
    #[error("randomx_create_vm returned null")]
    VmCreateFailed,
}

/* ======== RAII wrappers dla cache/dataset/vm ======== */

struct Cache {
    ptr: NonNull<randomx_cache>,
}

impl Cache {
    unsafe fn new(flags: randomx_flags, key: &[u8]) -> Result<Self, RandomxError> {
        let cache_ptr = randomx_alloc_cache(flags);
        let ptr = NonNull::new(cache_ptr).ok_or(RandomxError::CacheAllocFailed)?;
        randomx_init_cache(ptr.as_ptr(), key.as_ptr() as *const c_void, key.len() as c_size_t);
        Ok(Self { ptr })
    }

    fn as_mut_ptr(&self) -> *mut randomx_cache {
        self.ptr.as_ptr()
    }
}

impl Drop for Cache {
    fn drop(&mut self) {
        unsafe {
            randomx_release_cache(self.ptr.as_ptr());
        }
    }
}

struct Dataset {
    ptr: NonNull<randomx_dataset>,
}

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

impl Drop for Dataset {
    fn drop(&mut self) {
        unsafe {
            randomx_release_dataset(self.ptr.as_ptr());
        }
    }
}

struct Vm {
    ptr: NonNull<randomx_vm>,
}

impl Vm {
    unsafe fn new(flags: randomx_flags, cache: &Cache, dataset: &Dataset) -> Result<Self, RandomxError> {
        let vm_ptr = randomx_create_vm(flags, cache.as_mut_ptr(), dataset.as_mut_ptr());
        let ptr = NonNull::new(vm_ptr).ok_or(RandomxError::VmCreateFailed)?;
        Ok(Self { ptr })
    }

    fn as_mut_ptr(&self) -> *mut randomx_vm {
        self.ptr.as_ptr()
    }
}

impl Drop for Vm {
    fn drop(&mut self) {
        unsafe {
            randomx_destroy_vm(self.ptr.as_ptr());
        }
    }
}

/* ======== Wewnętrzne środowisko RandomX ======== */

struct RandomXEnv {
    _flags: randomx_flags,
    _cache: Cache,
    _dataset: Dataset,
    vm: Vm,
}

impl RandomXEnv {
    fn new_with_key(key: &[u8], secure: bool) -> Result<Self, RandomxError> {
        unsafe {
            let mut flags = randomx_get_flags();
            flags |= RANDOMX_FLAG_FULL_MEM | RANDOMX_FLAG_JIT;
            if secure {
                flags |= RANDOMX_FLAG_SECURE;
            }

            let cache = Cache::new(flags, key)?;
            let dataset = Dataset::new(flags, &cache)?;
            let vm = Vm::new(flags, &cache, &dataset)?;

            Ok(Self {
                _flags: flags,
                _cache: cache,
                _dataset: dataset,
                vm,
            })
        }
    }

    fn hash(&mut self, input: &[u8]) -> [u8; 32] {
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

/* ======== Publiczny API: RandomXHasher (tak jak wcześniej) ======== */

/// Publiczny hasher, używany w reszcie kodu.
/// Trzyma środowisko RandomX w Mutex, żeby metody mogły mieć `&self`.
pub struct RandomXHasher {
    env: Mutex<RandomXEnv>,
    epoch: u64,
}

impl RandomXHasher {
    /// Tworzy nowy hasher dla danej epoki.
    ///
    /// Kluczem do RandomX jest tu po prostu `epoch.to_le_bytes()`.
    /// Jeśli chcesz bardziej złożony seed (np. KMAC z net_id),
    /// można to łatwo zmienić.
    pub fn new(epoch: u64) -> Self {
        let key = epoch.to_le_bytes();
        let env = RandomXEnv::new_with_key(&key, false)
            .expect("RandomXEnv init failed (check librandomx linking)");

        Self {
            env: Mutex::new(env),
            epoch,
        }
    }

    /// Zwraca hash RandomX(input).
    pub fn hash(&self, input: &[u8]) -> [u8; 32] {
        let mut env = self.env.lock().expect("RandomXEnv mutex poisoned");
        env.hash(input)
    }

    /// Sprawdza, czy hash(input) == expected.
    pub fn verify(&self, input: &[u8], expected: &[u8; 32]) -> bool {
        let h = self.hash(input);
        &h == expected
    }

    /// Zwraca epokę, dla której skonstruowano hasher.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
}

/* ======== Funkcje pomocnicze dla PoW ======== */

/// Compare hashes (little-endian, jak w starej wersji)
fn hash_less_than(a: &[u8; 32], b: &[u8; 32]) -> bool {
    for i in (0..32).rev() {
        if a[i] < b[i] {
            return true;
        } else if a[i] > b[i] {
            return false;
        }
    }
    false
}

/// Mining function (proof-of-work)
///
/// Find nonce such that hash(data || nonce) < target
pub fn mine_randomx(
    hasher: &RandomXHasher,
    data: &[u8],
    target: &[u8; 32],
    max_iterations: u64,
) -> Option<(u64, [u8; 32])> {
    let start = Instant::now();

    for nonce in 0..max_iterations {
        // Combine data + nonce
        let mut input = data.to_vec();
        input.extend_from_slice(&nonce.to_le_bytes());

        // Hash
        let hash = hasher.hash(&input);

        // Check if hash < target
        if hash_less_than(&hash, target) {
            println!(
                "✅ Found nonce {} in {:.2}s",
                nonce,
                start.elapsed().as_secs_f64()
            );
            return Some((nonce, hash));
        }

        if nonce % 10 == 0 {
            print!(".");
            let _ = std::io::Write::flush(&mut std::io::stdout());
        }
    }

    println!("\n❌ No solution found after {} iterations", max_iterations);
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Wymaga biblioteki RandomX
    fn test_hash_deterministic() {
        let hasher = RandomXHasher::new(0);
        let data = b"test block data";
        let h1 = hasher.hash(data);
        let h2 = hasher.hash(data);
        assert_eq!(h1, h2);
    }
}
