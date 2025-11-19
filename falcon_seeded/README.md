# falcon_seeded

Deterministic Falcon-512 signatures via PQClean + KMAC-DRBG.

## Usage

```rust
use falcon_seeded::{keypair_with, sign_with, verify};

let drbg = Arc::new(MyDRBG::new(seed));
let (pk, sk) = keypair_with(drbg.clone())?;
let sig = sign_with(drbg, &sk, message)?;
assert!(verify(&pk, message, &sig));
```

## Build

Requires:
- Rust 1.91.1+
- C compiler (for PQClean sources)

Build script compiles PQClean Falcon-512 with KMAC-DRBG override.

## Status

**Research implementation.** Not audited.

## License

Apache 2.0
