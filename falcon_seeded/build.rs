// Build script for falcon_seeded with PQClean Falcon-512

use std::env;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let pqclean_falcon = manifest_dir.join("pqclean/crypto_sign/falcon-512/clean");
    let pqclean_common = manifest_dir.join("pqclean/common");

    // Check if PQClean sources exist
    if pqclean_falcon.exists() && pqclean_common.exists() {
        println!("cargo:warning=Building with PQClean Falcon-512 (full implementation)");
        build_pqclean(&pqclean_falcon, &pqclean_common);
    } else {
        println!("cargo:warning=PQClean not found. Using stub implementation.");
        println!("cargo:warning=Run: bash scripts/setup_pqclean.sh");
        build_stub();
    }

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=pqclean/");
}

fn build_pqclean(falcon_dir: &Path, common_dir: &Path) {
    let mut build = cc::Build::new();
    
    // Add include paths
    build.include(falcon_dir);
    build.include(common_dir);
    
    // Falcon-512 source files
    build.file(falcon_dir.join("codec.c"));
    build.file(falcon_dir.join("common.c"));
    build.file(falcon_dir.join("fft.c"));
    build.file(falcon_dir.join("fpr.c"));
    build.file(falcon_dir.join("keygen.c"));
    build.file(falcon_dir.join("rng.c"));
    build.file(falcon_dir.join("sign.c"));
    build.file(falcon_dir.join("vrfy.c"));
    build.file(falcon_dir.join("pqclean.c")); // API functions
    
    // SHAKE256 and randombytes from PQClean common
    build.file(common_dir.join("fips202.c"));
    build.file(common_dir.join("sp800-185.c"));
    build.file(common_dir.join("randombytes.c"));
    
    // Optimization flags
    build.opt_level(3);
    build.flag("-march=native");
    build.flag("-O3");
    build.flag("-fomit-frame-pointer");
    
    // Compile
    build.compile("pqclean_falcon512_clean");
    
    // Create wrapper C file for our API
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let wrapper = r#"
#include <stdlib.h>
#include <string.h>
#include "api.h"

// Wrapper functions that match our Rust FFI expectations

int tt_falcon512_keypair_seeded(
    unsigned char *pk,
    unsigned char *sk,
    void (*fill)(unsigned char*, size_t))
{
    // PQClean uses randombytes() internally
    // For now, call the standard keygen (uses OS randomness)
    // TODO: Implement proper seeded keygen
    (void)fill;
    return PQCLEAN_FALCON512_CLEAN_crypto_sign_keypair(pk, sk);
}

int tt_falcon512_sign_seeded(
    unsigned char *sig,
    size_t *siglen,
    const unsigned char *m,
    size_t mlen,
    const unsigned char *sk,
    void (*fill)(unsigned char*, size_t))
{
    (void)fill;
    size_t siglen_temp = *siglen;
    int ret = PQCLEAN_FALCON512_CLEAN_crypto_sign(sig, &siglen_temp, m, mlen, sk);
    *siglen = siglen_temp;
    return ret;
}

int tt_falcon512_verify(
    const unsigned char *sig,
    size_t siglen,
    const unsigned char *m,
    size_t mlen,
    const unsigned char *pk)
{
    unsigned char *msg_out = NULL;
    size_t mlen_out = 0;
    
    // PQClean's verify expects signed message format
    // Allocate buffer for message extraction
    size_t sm_len = siglen + mlen;
    unsigned char *sm = malloc(sm_len);
    if (!sm) return -1;
    
    // Construct signed message: sig || m
    memcpy(sm, sig, siglen);
    memcpy(sm + siglen, m, mlen);
    
    // Allocate output buffer
    msg_out = malloc(sm_len);
    if (!msg_out) {
        free(sm);
        return -1;
    }
    
    int ret = PQCLEAN_FALCON512_CLEAN_crypto_sign_open(
        msg_out, &mlen_out, sm, sm_len, pk
    );
    
    free(sm);
    free(msg_out);
    
    return ret;
}
"#;
    
    let wrapper_path = out_dir.join("falcon_wrapper.c");
    std::fs::write(&wrapper_path, wrapper).expect("Failed to write wrapper");
    
    cc::Build::new()
        .include(falcon_dir)
        .include(common_dir)
        .file(&wrapper_path)
        .compile("falcon_wrapper");
}

fn build_stub() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    
    let stub_c = r#"
#include <stddef.h>
#include <string.h>

// Stub implementation - returns errors

int tt_falcon512_keypair_seeded(
    unsigned char *pk,
    unsigned char *sk,
    void (*fill)(unsigned char*, size_t))
{
    (void)fill;
    memset(pk, 0, 897);
    memset(sk, 0, 1281);
    return -1; // Not implemented
}

int tt_falcon512_sign_seeded(
    unsigned char *sig,
    size_t *siglen,
    const unsigned char *m,
    size_t mlen,
    const unsigned char *sk,
    void (*fill)(unsigned char*, size_t))
{
    (void)m; (void)mlen; (void)sk; (void)fill;
    *siglen = 0;
    return -1; // Not implemented
}

int tt_falcon512_verify(
    const unsigned char *sig,
    size_t siglen,
    const unsigned char *m,
    size_t mlen,
    const unsigned char *pk)
{
    (void)sig; (void)siglen; (void)m; (void)mlen; (void)pk;
    return -1; // Not implemented
}
"#;
    
    let stub_path = out_dir.join("falcon_stub.c");
    std::fs::write(&stub_path, stub_c).expect("Failed to write stub");
    
    cc::Build::new()
        .file(&stub_path)
        .compile("falcon_seeded");
}
