#![forbid(unsafe_code)]

use rand::RngCore;

fn main() {
    println!("\nâ•”â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•—");
    println!("â•‘  E2E Test: Alice â†’ Bob Transaction over PQ P2P        â•‘");
    println!("â•šâ•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•ť\n");

    println!("đź”‘ Generowanie kluczy...");
    let (alice_falcon_pk, alice_falcon_sk) = tt_node::falcon_sigs::falcon_keypair();
    let (alice_kyber_pk,  alice_kyber_sk)  = tt_node::kyber_kem::kyber_keypair();
    let alice_id = tt_node::p2p::secure::NodeIdentity::from_keys(alice_falcon_pk, alice_falcon_sk, alice_kyber_pk,  alice_kyber_sk);
    println!("  âś… Alice ID: {}", hex::encode(&alice_id.node_id[..8]));

    let (bob_falcon_pk, bob_falcon_sk) = tt_node::falcon_sigs::falcon_keypair();
    let (bob_kyber_pk,  bob_kyber_sk)  = tt_node::kyber_kem::kyber_keypair();
    let bob_id = tt_node::p2p::secure::NodeIdentity::from_keys(bob_falcon_pk, bob_falcon_sk, bob_kyber_pk,  bob_kyber_sk);
    println!("  âś… Bob ID: {}", hex::encode(&bob_id.node_id[..8]));

    println!("\nđź’¸ Alice tworzy transakcjÄ™ dla Bob...");
    let mut blinding = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut blinding);
    let recipient = tt_node::core::bytes32(tt_node::falcon_sigs::falcon_pk_to_bytes(&bob_id.falcon_pk));
    let output = tt_node::tx_stark::TxOutputStark::new(100_000, &blinding, recipient, &bob_id.kyber_pk);
    assert!(output.verify());
    println!("  âś… TxOutputStark utworzony (100,000 tokens)");

    let tx = tt_node::tx_stark::TransactionStark { inputs: vec![], outputs: vec![output.clone()], fee: 0, nonce: 1, timestamp: tt_node::core::now_ts() };
    let (valid, total) = tx.verify_all_proofs();
    assert_eq!(valid, total);
    println!("  âś… Transakcja zweryfikowana ({}/{} proofs OK)", valid, total);
    let tx_bytes = tx.to_bytes();

    println!("\nđź¤ť PQC Handshake (Kyber + Falcon)...");
    let (ch, transcript_client0) = tt_node::p2p::secure::build_client_hello(&alice_id, tt_node::p2p::secure::PROTOCOL_VERSION).expect("CH failed");
    let (sh, session_key_server, transcript_server1) = tt_node::p2p::secure::handle_client_hello(&bob_id, &ch, tt_node::p2p::secure::PROTOCOL_VERSION, transcript_client0.clone_state()).expect("SH failed");
    let (session_key_client, transcript_client1) = tt_node::p2p::secure::handle_server_hello(&alice_id, &ch, &sh, transcript_client0, tt_node::p2p::secure::PROTOCOL_VERSION).expect("handle SH failed");
    let (cf, _) = tt_node::p2p::secure::build_client_finished(&alice_id, transcript_client1).expect("CF failed");
    let _ = tt_node::p2p::secure::verify_client_finished(&ch.falcon_pk, transcript_server1, &cf).expect("verify CF failed");
    assert_eq!(session_key_client.as_bytes(), session_key_server.as_bytes());
    println!("  đź” Session key established!");

    println!("\nđź“ˇ WysyĹ‚anie transakcji...");
    let mut chan_alice = tt_node::p2p::secure::SecureChannel::new(session_key_client);
    let mut chan_bob = tt_node::p2p::secure::SecureChannel::new(session_key_server);
    let ciphertext = chan_alice.encrypt(&tx_bytes, b"TX_STARK_PQC").expect("encrypt failed");
    let decrypted = chan_bob.decrypt(&ciphertext, b"TX_STARK_PQC").expect("decrypt failed");
    assert_eq!(decrypted, tx_bytes);

    println!("\nđź’° Bob weryfikuje i odszyfrowuje wartoĹ›Ä‡...");
    let rx_tx = tt_node::tx_stark::TransactionStark::from_bytes(&decrypted).expect("TX deser failed");
    let (valid2, total2) = rx_tx.verify_all_proofs();
    assert_eq!(valid2, total2);
    let value = rx_tx.outputs[0].decrypt_and_verify(&bob_id.kyber_sk).expect("decrypt failed");
    assert_eq!(value, 100_000u64);
    println!("  đź’Ž Bob odszyfrowaĹ‚: {} tokens", value);

    println!("\nâ•”â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•—");
    println!("â•‘  âś… E2E TEST PASSED! All systems operational!         â•‘");
    println!("â•šâ•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•ť\n");
}
