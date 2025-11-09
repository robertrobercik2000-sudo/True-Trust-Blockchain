use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=c/");
    println!("cargo:rerun-if-changed=pqclean/");

    let mut cc = cc::Build::new();

    // Our shim with RNG + FFI glue
    cc.file("c/randombytes_kmac.c");
    cc.file("c/falcon_shim.c");

    // Add Falcon sources from PQClean (clean variant)
    // Copy PQClean/crypto_sign/falcon-512/clean to falcon_seeded/pqclean/crypto_sign/falcon-512/clean
    let base: PathBuf = ["pqclean", "crypto_sign", "falcon-512", "clean"]
        .iter()
        .collect();

    // Check if PQClean sources exist
    if !base.exists() {
        eprintln!("WARNING: PQClean Falcon sources not found at {:?}", base);
        eprintln!("Please run: ./scripts/setup_pqclean.sh");
        eprintln!("Or manually copy PQClean/crypto_sign/falcon-512/clean/ to falcon_seeded/pqclean/crypto_sign/falcon-512/clean/");
        
        // Create placeholder to prevent build failure
        std::fs::create_dir_all(&base).ok();
        
        // Exit early - user needs to setup PQClean
        panic!("PQClean Falcon sources required. See falcon_seeded/README.md for setup instructions.");
    }

    // ✅ Updated to match actual PQClean Falcon-512 file names
    for f in [
        "pqclean.c", "codec.c", "common.c", "fft.c", "fpr.c",
        "keygen.c", "rng.c", "sign.c", "vrfy.c",
    ] {
        let path = base.join(f);
        if path.exists() {
            cc.file(&path);
        } else {
            panic!("Missing PQClean file: {:?}. Run setup_pqclean.sh", path);
        }
    }

    // Disable built-in randombytes.c from Falcon and replace with ours
    cc.define("PQCLEAN_FALCON512_CLEAN_NAMESPACE", None);

    cc.include(&base);
    cc.include("c");
    cc.flag_if_supported("-O3");
    cc.flag_if_supported("-march=native");
    cc.flag_if_supported("-fomit-frame-pointer");
    
    // Compile
    cc.compile("falcon_seeded");
}
