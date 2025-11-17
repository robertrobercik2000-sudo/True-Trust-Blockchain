// build.rs – bindowanie RandomX (PRO) - CONDITIONAL
//
// RandomX jest używany tylko przez node (consensus), NIE przez wallet CLI!
// Link tylko jeśli feature "randomx-ffi" jest enabled.

use std::env;

fn main() {
    // Check if randomx-ffi feature is enabled
    // Cargo sets CARGO_FEATURE_<name> for each enabled feature
    let randomx_enabled = env::var("CARGO_FEATURE_RANDOMX_FFI").is_ok();
    
    if !randomx_enabled {
        println!("cargo:warning=RandomX FFI disabled (not needed for wallet CLI)");
        return;
    }
    
    println!("cargo:warning=RandomX FFI enabled (for consensus node)");
    
    // Jeśli zmienisz te zmienne środowiskowe – przebuduj projekt
    println!("cargo:rerun-if-env-changed=RANDOMX_LIB_DIR");
    println!("cargo:rerun-if-env-changed=RANDOMX_STATIC");

    // 1) Najpierw spróbuj użyć pkg-config, jeśli jest dostępny
    //    (np. po instalacji librandomx-dev z systemowego repo).
    if let Ok(lib) = pkg_config::Config::new()
        .atleast_version("1.0")
        .probe("randomx")
    {
        // Ścieżki do bibliotek znalezione przez pkg-config
        for path in lib.link_paths {
            println!("cargo:rustc-link-search=native={}", path.display());
        }
        // Nazwy bibliotek (zwykle: "randomx")
        for libname in lib.libs {
            println!("cargo:rustc-link-lib={}", libname);
        }
        println!("cargo:warning=RandomX found via pkg-config ✅");
        return;
    }

    // 2) Fallback: użyj zmiennej środowiskowej albo standardowych katalogów
    if let Ok(dir) = env::var("RANDOMX_LIB_DIR") {
        // ręcznie wskazane położenie np. /home/user/randomx/build
        println!("cargo:rustc-link-search=native={dir}");
        println!("cargo:warning=Using RANDOMX_LIB_DIR={dir}");
    } else {
        // typowe ścieżki w Linuksie
        println!("cargo:rustc-link-search=native=/usr/local/lib");
        println!("cargo:rustc-link-search=native=/usr/lib");
        println!("cargo:warning=Searching RandomX in standard paths (/usr/local/lib, /usr/lib)");
    }

    // 3) Static vs dynamic
    let kind = if env::var("RANDOMX_STATIC").as_deref() == Ok("1") {
        println!("cargo:warning=Linking RandomX statically");
        "static"
    } else {
        println!("cargo:warning=Linking RandomX dynamically");
        "dylib"
    };

    // Podlinkuj librandomx
    println!("cargo:rustc-link-lib={kind}=randomx");
    
    println!("cargo:warning=⚠️  If build fails, install RandomX:");
    println!("cargo:warning=   git clone https://github.com/tevador/RandomX");
    println!("cargo:warning=   cd RandomX && mkdir build && cd build");
    println!("cargo:warning=   cmake .. && make && sudo make install");
}
