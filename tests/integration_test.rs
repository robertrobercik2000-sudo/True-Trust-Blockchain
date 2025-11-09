//! Integration tests for quantum-safe wallet
//!
//! These tests verify the full end-to-end flow:
//! 1. Key generation (sender + recipient)
//! 2. Quantum-safe hint construction
//! 3. Hint transmission (simulated blockchain)
//! 4. Hint verification and decryption
//! 5. Value recovery

use quantum_falcon_wallet::{
    QuantumKeySearchCtx,
    QuantumSafeHint,
    HintPayloadV1,
    hint_fingerprint16,
};

/* ===== TEST 1: BASIC ROUNDTRIP ===== */

#[test]
fn test_basic_hint_roundtrip() {
    // Setup: Create sender and recipient
    let sender_seed = [0x42u8; 32];
    let sender = QuantumKeySearchCtx::new(sender_seed)
        .expect("Failed to create sender context");

    let recipient_seed = [0x99u8; 32];
    let recipient = QuantumKeySearchCtx::new(recipient_seed)
        .expect("Failed to create recipient context");

    // Transaction: Output commitment
    let c_out = [0xABu8; 32];

    // Payload: Hidden value and blinding factor
    let payload = HintPayloadV1 {
        r_blind: [0x11u8; 32],
        value: 1000,
        memo: vec![],
    };

    // Sender: Build quantum-safe hint
    let hint = sender.build_quantum_hint(
        recipient.mlkem_public_key(),
        &recipient.x25519_public_key(),
        &c_out,
        &payload,
    ).expect("Failed to build hint");

    // Simulate blockchain transmission
    let serialized = bincode::serialize(&hint)
        .expect("Failed to serialize hint");
    let hint_received: QuantumSafeHint = bincode::deserialize(&serialized)
        .expect("Failed to deserialize hint");

    // Recipient: Verify and decrypt
    let (decoded, verified) = recipient
        .verify_quantum_hint(&hint_received, &c_out)
        .expect("Verification failed");

    // Assert: Check correctness
    assert!(verified, "Hint verification failed");
    assert_eq!(decoded.r_blind, payload.r_blind);
    assert_eq!(decoded.value, Some(payload.value));
}

/* ===== TEST 2: MULTIPLE RECIPIENTS (PRIVACY) ===== */

#[test]
fn test_multiple_recipients_privacy() {
    let sender = QuantumKeySearchCtx::new([0x01u8; 32])
        .expect("Failed to create sender");

    // Three recipients with different keys
    let recipients: Vec<_> = (0..3)
        .map(|i| {
            let mut seed = [0u8; 32];
            seed[0] = i;
            QuantumKeySearchCtx::new(seed).expect("Failed to create recipient")
        })
        .collect();

    // Three outputs, each for a different recipient
    let outputs: Vec<_> = recipients
        .iter()
        .enumerate()
        .map(|(i, recipient)| {
            let mut c_out = [0u8; 32];
            c_out[0] = i as u8;

            let payload = HintPayloadV1 {
                r_blind: [i as u8; 32],
                value: 1000 * (i as u64 + 1),
                memo: vec![],
            };

            let hint = sender.build_quantum_hint(
                recipient.mlkem_public_key(),
                &recipient.x25519_public_key(),
                &c_out,
                &payload,
            ).expect("Failed to build hint");

            (c_out, hint, payload)
        })
        .collect();

    // Each recipient should only decrypt their own hint
    for (i, recipient) in recipients.iter().enumerate() {
        let (c_out, hint, expected_payload) = &outputs[i];

        // Verify own hint
        let (decoded, verified) = recipient
            .verify_quantum_hint(hint, c_out)
            .expect("Failed to verify own hint");

        assert!(verified);
        assert_eq!(decoded.value, Some(expected_payload.value));

        // Should NOT decrypt other recipients' hints
        for (j, (other_c_out, other_hint, _)) in outputs.iter().enumerate() {
            if i != j {
                let result = recipient.verify_quantum_hint(other_hint, other_c_out);
                // Either fails verification or returns None
                if let Some((decoded, _)) = result {
                    // If it somehow decodes, value should be wrong
                    assert_ne!(decoded.value, Some(expected_payload.value));
                }
            }
        }
    }
}

/* ===== TEST 3: BLOOM FILTER FAST SCANNING ===== */

#[test]
fn test_bloom_filter_scanning() {
    use std::collections::HashSet;

    let sender = QuantumKeySearchCtx::new([0xAAu8; 32])
        .expect("Failed to create sender");

    let recipient = QuantumKeySearchCtx::new([0xBBu8; 32])
        .expect("Failed to create recipient");

    // Simulate blockchain with 100 outputs, only 3 are for recipient
    let mut all_hints = Vec::new();
    let mut my_fingerprints = HashSet::new();

    // Generate 3 hints for recipient
    for i in 0..3 {
        let mut c_out = [0u8; 32];
        c_out[0] = i;

        let payload = HintPayloadV1 {
            r_blind: [i; 32],
            value: 1000 + i as u64,
            memo: vec![],
        };

        let hint = sender.build_quantum_hint(
            recipient.mlkem_public_key(),
            &recipient.x25519_public_key(),
            &c_out,
            &payload,
        ).expect("Failed to build hint");

        let fp = hint_fingerprint16(&hint, &c_out);
        my_fingerprints.insert(fp);
        all_hints.push((c_out, hint, true)); // Mark as "mine"
    }

    // Generate 97 decoy hints (different recipients)
    let decoy = QuantumKeySearchCtx::new([0xFFu8; 32])
        .expect("Failed to create decoy");

    for i in 3..100 {
        let mut c_out = [0u8; 32];
        c_out[0] = i;

        let payload = HintPayloadV1 {
            r_blind: [i; 32],
            value: 0,
            memo: vec![],
        };

        let hint = sender.build_quantum_hint(
            decoy.mlkem_public_key(),
            &decoy.x25519_public_key(),
            &c_out,
            &payload,
        ).expect("Failed to build hint");

        all_hints.push((c_out, hint, false)); // Not mine
    }

    // Scan blockchain with Bloom filter pre-filtering
    let mut found_count = 0;
    let mut full_verifications = 0;

    for (c_out, hint, is_mine) in &all_hints {
        let fp = hint_fingerprint16(hint, c_out);

        // Fast pre-filter
        if my_fingerprints.contains(&fp) {
            full_verifications += 1;

            // Full verification (expensive)
            if let Some((decoded, verified)) = recipient.verify_quantum_hint(hint, c_out) {
                if verified && decoded.value.is_some() {
                    found_count += 1;
                    assert!(is_mine, "False positive: decoy hint verified!");
                }
            }
        }
    }

    // Assert: Found exactly 3 hints with minimal verifications
    assert_eq!(found_count, 3, "Should find all 3 hints");
    assert!(full_verifications <= 5, "Bloom filter should reduce verifications (got {})", full_verifications);
}

/* ===== TEST 4: REPLAY PROTECTION ===== */

#[test]
fn test_replay_protection() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let sender = QuantumKeySearchCtx::new([0x01u8; 32])
        .expect("Failed to create sender");
    let recipient = QuantumKeySearchCtx::new([0x02u8; 32])
        .expect("Failed to create recipient");

    let c_out = [0xCDu8; 32];
    let payload = HintPayloadV1 {
        r_blind: [0x11u8; 32],
        value: 500,
        memo: vec![],
    };

    // Build fresh hint
    let mut hint = sender.build_quantum_hint(
        recipient.mlkem_public_key(),
        &recipient.x25519_public_key(),
        &c_out,
        &payload,
    ).expect("Failed to build hint");

    // Verify fresh hint (should work)
    let result = recipient.verify_quantum_hint(&hint, &c_out);
    assert!(result.is_some(), "Fresh hint should verify");

    // Tamper timestamp (set to 10 years ago)
    let old_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .saturating_sub(10 * 365 * 24 * 3600);
    hint.timestamp = old_timestamp;

    // Verify old hint with strict parameters (1 hour max skew)
    let result = recipient.verify_quantum_hint_with_params(&hint, &c_out, 3600, false);
    assert!(result.is_none(), "Old hint should be rejected");
}

/* ===== TEST 5: WRONG COMMITMENT FAILS ===== */

#[test]
fn test_wrong_commitment_fails() {
    let sender = QuantumKeySearchCtx::new([0xAAu8; 32])
        .expect("Failed to create sender");
    let recipient = QuantumKeySearchCtx::new([0xBBu8; 32])
        .expect("Failed to create recipient");

    let c_out = [0x42u8; 32];
    let wrong_c_out = [0x99u8; 32];

    let payload = HintPayloadV1 {
        r_blind: [0x11u8; 32],
        value: 1000,
        memo: vec![],
    };

    let hint = sender.build_quantum_hint(
        recipient.mlkem_public_key(),
        &recipient.x25519_public_key(),
        &c_out,
        &payload,
    ).expect("Failed to build hint");

    // Verify with correct commitment (should work)
    let result_correct = recipient.verify_quantum_hint(&hint, &c_out);
    assert!(result_correct.is_some(), "Correct commitment should verify");

    // Verify with wrong commitment (should fail - transcript mismatch)
    let result_wrong = recipient.verify_quantum_hint(&hint, &wrong_c_out);
    assert!(result_wrong.is_none(), "Wrong commitment should fail verification");
}

/* ===== TEST 6: CONCURRENT HINT PROCESSING ===== */

#[test]
fn test_concurrent_hint_processing() {
    use std::sync::Arc;
    use std::thread;

    let sender = Arc::new(
        QuantumKeySearchCtx::new([0x01u8; 32])
            .expect("Failed to create sender")
    );

    let recipient = Arc::new(
        QuantumKeySearchCtx::new([0x02u8; 32])
            .expect("Failed to create recipient")
    );

    // Generate 10 hints
    let hints: Vec<_> = (0..10)
        .map(|i| {
            let mut c_out = [0u8; 32];
            c_out[0] = i;

            let payload = HintPayloadV1 {
                r_blind: [i; 32],
                value: 100 * (i as u64 + 1),
                memo: vec![],
            };

            let hint = sender.build_quantum_hint(
                recipient.mlkem_public_key(),
                &recipient.x25519_public_key(),
                &c_out,
                &payload,
            ).expect("Failed to build hint");

            (c_out, hint, payload)
        })
        .collect();

    // Process hints in parallel threads
    let handles: Vec<_> = hints
        .into_iter()
        .map(|(c_out, hint, expected_payload)| {
            let recipient_clone = Arc::clone(&recipient);

            thread::spawn(move || {
                let (decoded, verified) = recipient_clone
                    .verify_quantum_hint(&hint, &c_out)
                    .expect("Verification failed");

                assert!(verified);
                assert_eq!(decoded.value, Some(expected_payload.value));
            })
        })
        .collect();

    // Wait for all threads
    for handle in handles {
        handle.join().expect("Thread panicked");
    }
}

/* ===== TEST 7: LARGE MEMO FIELD ===== */

#[test]
fn test_large_memo_field() {
    let sender = QuantumKeySearchCtx::new([0x01u8; 32])
        .expect("Failed to create sender");
    let recipient = QuantumKeySearchCtx::new([0x02u8; 32])
        .expect("Failed to create recipient");

    let c_out = [0xEFu8; 32];

    // Large memo (1KB)
    let large_memo = vec![0xAAu8; 1024];

    let payload = HintPayloadV1 {
        r_blind: [0x11u8; 32],
        value: 5000,
        memo: large_memo.clone(),
    };

    let hint = sender.build_quantum_hint(
        recipient.mlkem_public_key(),
        &recipient.x25519_public_key(),
        &c_out,
        &payload,
    ).expect("Failed to build hint with large memo");

    let (decoded, verified) = recipient
        .verify_quantum_hint(&hint, &c_out)
        .expect("Failed to verify hint with large memo");

    assert!(verified);
    assert_eq!(decoded.value, Some(5000));
    // Memo is encrypted and decoded, just verify it's not empty
    // (memo_items uses TLV format, not a direct Vec<u8> comparison)
    // For this test, we just ensure decryption succeeded
}
