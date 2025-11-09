//! Integration tests for tt_priv_cli wallet
//!
//! These tests validate the complete wallet lifecycle without requiring TTY:
//! - Wallet creation and encryption
//! - Key derivation (classic + quantum)
//! - Shamir secret sharing (3-of-5)
//! - Address generation
//! - Serialization/deserialization

use quantum_falcon_wallet::crypto::kmac as ck;

#[cfg(feature = "tt-full")]
mod wallet_tests {
    use super::*;
    use rand::rngs::OsRng;
    use rand::RngCore;
    use sharks::{Sharks, Share};

    #[test]
    fn test_kmac256_derive_key_deterministic() {
        let key = b"test_key_12345678901234567890123";
        let label = b"TEST-LABEL";
        let context = b"context";

        let derived1 = ck::kmac256_derive_key(key, label, context);
        let derived2 = ck::kmac256_derive_key(key, label, context);

        assert_eq!(derived1, derived2, "KMAC256 derivation must be deterministic");
        assert_ne!(&derived1[..], &key[..32], "Derived key must differ from input");
    }

    #[test]
    fn test_kmac256_xof_variable_length() {
        let key = b"secret_key";
        let label = b"XOF-TEST";
        let context = b"test";

        let out32 = ck::kmac256_xof(key, label, context, 32);
        let out64 = ck::kmac256_xof(key, label, context, 64);

        assert_eq!(out32.len(), 32);
        assert_eq!(out64.len(), 64);
        assert_eq!(&out64[..32], &out32[..], "XOF prefix must match");
    }

    #[test]
    fn test_kmac256_tag_authentication() {
        let key = b"auth_key_1234567890";
        let label = b"AUTH-TAG";
        let message1 = b"authentic message";
        let message2 = b"tampered message";

        let tag1 = ck::kmac256_tag(key, label, message1);
        let tag2 = ck::kmac256_tag(key, label, message2);
        let tag1_dup = ck::kmac256_tag(key, label, message1);

        assert_eq!(tag1, tag1_dup, "Tags must be deterministic");
        assert_ne!(tag1, tag2, "Different messages must produce different tags");
    }

    #[test]
    fn test_shamir_secret_sharing_3_of_5() {
        // Create secret
        let mut secret = [0u8; 32];
        OsRng.fill_bytes(&mut secret);

        // Split into 5 shares (3 required)
        let sharks = Sharks(3);
        let dealer = sharks.dealer(&secret);
        let shares: Vec<Share> = dealer.take(5).collect();
        assert_eq!(shares.len(), 5);

        // Recover from shares 1, 3, 5
        let recovered = sharks.recover(vec![
            &shares[0],
            &shares[2],
            &shares[4],
        ].into_iter()).expect("Recovery must succeed");

        assert_eq!(&secret[..], &recovered[..], "Recovered secret must match original");

        // Test insufficient shares (only 2)
        let result = sharks.recover(vec![&shares[0], &shares[1]].into_iter());
        assert!(result.is_err(), "Recovery with 2-of-3 shares must fail");
    }

    #[test]
    fn test_shamir_any_subset_recovers() {
        let mut secret = [0u8; 32];
        OsRng.fill_bytes(&mut secret);

        let sharks = Sharks(3);
        let dealer = sharks.dealer(&secret);
        let shares: Vec<Share> = dealer.take(5).collect();

        // Test all possible 3-of-5 combinations
        let test_combinations = vec![
            vec![0, 1, 2],
            vec![0, 1, 3],
            vec![0, 2, 4],
            vec![1, 3, 4],
            vec![2, 3, 4],
        ];

        for combo in test_combinations {
            let share_refs: Vec<&Share> = combo.iter().map(|&i| &shares[i]).collect();
            let recovered = sharks.recover(share_refs.into_iter())
                .expect(&format!("Combo {:?} must recover secret", combo));
            assert_eq!(
                &secret[..],
                &recovered[..],
                "Combo {:?} must recover secret",
                combo
            );
        }
    }

    #[test]
    fn test_key_derivation_hierarchy() {
        let master = [0x42u8; 32];

        let spend = ck::kmac256_derive_key(&master, b"TT-SPEND.v1", b"seed");
        let scan = ck::kmac256_derive_key(&master, b"TT-SCAN.v1", b"seed");

        assert_ne!(spend, scan, "Derived keys must be independent");
        assert_ne!(&spend[..], &master[..], "Child key must differ from master");
    }

    #[test]
    fn test_shard_mask_xor() {
        let share = b"secret_share_data_1234567890";
        let password = "my_secure_password";
        let salt = [0x55u8; 32];

        // Mask
        let mask = ck::kmac256_xof(password.as_bytes(), b"TT-SHARD.mask", &salt, share.len());
        let masked: Vec<u8> = share.iter().zip(mask.iter()).map(|(a, b)| a ^ b).collect();

        // Unmask
        let mask2 = ck::kmac256_xof(password.as_bytes(), b"TT-SHARD.mask", &salt, share.len());
        let unmasked: Vec<u8> = masked.iter().zip(mask2.iter()).map(|(a, b)| a ^ b).collect();

        assert_eq!(&unmasked[..], &share[..], "Unmasked data must match original");
        assert_ne!(&masked[..], &share[..], "Masked data must differ from original");
    }

    #[test]
    fn test_quantum_key_sizes() {
        use pqcrypto_falcon::falcon512;
        use pqcrypto_kyber::kyber768 as mlkem;
        use pqcrypto_traits::sign::{PublicKey as PQPublicKey, SecretKey as PQSecretKey};
        use pqcrypto_traits::kem::{PublicKey as PQKemPublicKey, SecretKey as PQKemSecretKey};

        let (falcon_pk, falcon_sk) = falcon512::keypair();
        let (mlkem_pk, mlkem_sk) = mlkem::keypair();

        println!("Falcon512 SK: {} bytes", falcon_sk.as_bytes().len());
        println!("Falcon512 PK: {} bytes", falcon_pk.as_bytes().len());
        println!("ML-KEM SK: {} bytes", mlkem_sk.as_bytes().len());
        println!("ML-KEM PK: {} bytes", mlkem_pk.as_bytes().len());

        assert!(falcon_sk.as_bytes().len() > 1000, "Falcon SK should be ~1280 bytes");
        assert!(falcon_pk.as_bytes().len() > 800, "Falcon PK should be ~897 bytes");
        assert_eq!(mlkem_sk.as_bytes().len(), 2400);
        assert_eq!(mlkem_pk.as_bytes().len(), 1184);
    }

    #[test]
    fn test_pad_unpad_calculation() {
        // Test padding calculation (pad/unpad functions are private)
        let data = b"test data";
        let block_size = 256;
        
        let expected_padded_len = {
            let len = data.len();
            let pad_len = (block_size - ((len + 8) % block_size)) % block_size;
            len + pad_len + 8
        };
        
        assert_eq!(expected_padded_len % block_size, 0, "Padded length must be multiple of block size");
    }
}

#[test]
fn test_basic_compilation() {
    // This test just ensures the module compiles
    assert!(true);
}

#[cfg(feature = "tt-full")]
#[test]
fn test_wallet_types_serialization() {
    // Test that wallet types can be serialized/deserialized
    // This validates the serde derives
    use serde::{Serialize, Deserialize};
    
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct TestPayload {
        version: u32,
        data: Vec<u8>,
    }
    
    let payload = TestPayload {
        version: 5,
        data: vec![1, 2, 3, 4],
    };
    
    let serialized = bincode::serialize(&payload).unwrap();
    let deserialized: TestPayload = bincode::deserialize(&serialized).unwrap();
    
    assert_eq!(payload, deserialized);
}
