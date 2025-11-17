// Minimal, narrow shim for PQClean randomness: randombytes()
// Instead of OS RNG – use DRBG callback set from Rust

#include <stdint.h>
#include <stddef.h>

#ifdef _MSC_VER
__declspec(thread) static void (*tls_fill)(uint8_t*, size_t) = 0;
#else
__thread static void (*tls_fill)(uint8_t*, size_t) = 0;
#endif

// PQClean expects randombytes() symbol
void randombytes(uint8_t* out, size_t outlen) {
    if (tls_fill) {
        tls_fill(out, outlen);
    } else {
        // Fallback (should not happen in seeded mode)
        // Zero – causes verifiable failures instead of silent security breach
        for (size_t i = 0; i < outlen; i++) {
            out[i] = 0;
        }
    }
}

// PQClean namespace variant (required by newer pqclean.c)
void PQCLEAN_randombytes(uint8_t* out, size_t outlen) {
    randombytes(out, outlen);  // Forward to our implementation
}

// Set pointer to byte-filling function (provided from Rust)
void tt_set_randombytes(void (*fill_fn)(uint8_t*, size_t)) {
    tls_fill = fill_fn;
}
