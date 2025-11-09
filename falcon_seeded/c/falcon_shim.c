#include <stdint.h>
#include <stddef.h>

// Declarations of symbols from PQClean (clean variant)
int PQCLEAN_FALCON512_CLEAN_crypto_sign_keypair(uint8_t *pk, uint8_t *sk);
int PQCLEAN_FALCON512_CLEAN_crypto_sign_signature(
    uint8_t *sig, size_t *siglen,
    const uint8_t *m, size_t mlen,
    const uint8_t *sk
);
int PQCLEAN_FALCON512_CLEAN_crypto_sign_verify(
    const uint8_t *sig, size_t siglen,
    const uint8_t *m, size_t mlen,
    const uint8_t *pk
);

// Declared in randombytes_kmac.c
void tt_set_randombytes(void (*fill_fn)(uint8_t*, size_t));

// "Bare" functions called from Rust – with RNG already set
int tt_falcon512_keypair_seeded(
    uint8_t *pk,
    uint8_t *sk,
    void (*fill_fn)(uint8_t*, size_t)
) {
    tt_set_randombytes(fill_fn);
    return PQCLEAN_FALCON512_CLEAN_crypto_sign_keypair(pk, sk);
}

int tt_falcon512_sign_seeded(
    uint8_t *sig,
    size_t *siglen,
    const uint8_t *m,
    size_t mlen,
    const uint8_t *sk,
    void (*fill_fn)(uint8_t*, size_t)
) {
    tt_set_randombytes(fill_fn);
    return PQCLEAN_FALCON512_CLEAN_crypto_sign_signature(sig, siglen, m, mlen, sk);
}

int tt_falcon512_verify(
    const uint8_t *sig,
    size_t siglen,
    const uint8_t *m,
    size_t mlen,
    const uint8_t *pk
) {
    return PQCLEAN_FALCON512_CLEAN_crypto_sign_verify(sig, siglen, m, mlen, pk);
}
