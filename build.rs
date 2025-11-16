// build.rs - Linkowanie do oficjalnej biblioteki RandomX
//
// UWAGA: Ten build script zakłada, że biblioteka RandomX jest dostępna.
//
// Instalacja RandomX (Linux/macOS):
// 1. git clone https://github.com/tevador/RandomX
// 2. cd RandomX && mkdir build && cd build
// 3. cmake .. && make
// 4. sudo make install  (lub skopiuj librandomx.a do /usr/local/lib)
//
// Alternatywnie, ustaw zmienną środowiskową:
// export RANDOMX_LIB_DIR=/path/to/RandomX/build

fn main() {
    // Sprawdź czy user chce używać RandomX FFI
    let use_randomx_ffi = std::env::var("RANDOMX_FFI").unwrap_or_default() == "1";
    
    if !use_randomx_ffi {
        println!("cargo:warning=RandomX FFI disabled (set RANDOMX_FFI=1 to enable)");
        println!("cargo:warning=Using Pure Rust fallback (randomx_full.rs)");
        return;
    }
    
    println!("cargo:warning=Enabling RandomX FFI...");
    
    // Włącz feature flag
    println!("cargo:rustc-cfg=feature=\"randomx-ffi-enabled\"");
    
    println!("cargo:warning=Linking RandomX C library...");
    
    // Próbuj znaleźć bibliotekę
    if let Ok(lib_dir) = std::env::var("RANDOMX_LIB_DIR") {
        println!("cargo:rustc-link-search=native={}", lib_dir);
    } else {
        // Domyślne lokalizacje
        println!("cargo:rustc-link-search=native=/usr/local/lib");
        println!("cargo:rustc-link-search=native=/usr/lib");
    }
    
    // Link library
    println!("cargo:rustc-link-lib=randomx");
    
    // Rerun if environment changes
    println!("cargo:rerun-if-env-changed=RANDOMX_LIB_DIR");
    println!("cargo:rerun-if-env-changed=RANDOMX_FFI");
}
