//! Consensus rewards test example

fn main() {
    println!("CONSENSUS TEST");
    let mut c = tt_node::consensus_pro::ConsensusPro::new_default();
    let (apk,_) = tt_node::falcon_sigs::falcon_keypair();
    let a = tt_node::node_id::node_id_from_falcon_pk(&apk);
    let (bpk,_) = tt_node::falcon_sigs::falcon_keypair();
    let b = tt_node::node_id::node_id_from_falcon_pk(&bpk);
    let (fpk,_) = tt_node::falcon_sigs::falcon_keypair();
    let f = tt_node::node_id::node_id_from_falcon_pk(&fpk);
    c.register_validator(a, 1000000000u128);
    c.register_validator(b, 500000000u128);
    c.register_validator(f, 100000000u128);
    c.recompute_all_stake_q();
    c.record_quality_f64(&a, 0.80);
    c.record_quality_f64(&b, 0.95);
    c.record_quality_f64(&f, 0.60);
    c.update_all_trust();
    let aw = c.compute_validator_weight(&a).unwrap();
    let bw = c.compute_validator_weight(&b).unwrap();
    let fw = c.compute_validator_weight(&f).unwrap();
    let tw = aw+bw+fw;
    let r: u128 = 1000000;
    println!("Alice: {} tokens ({}%)", r*aw/tw, aw*100/tw);
    println!("Bob:   {} tokens ({}%)", r*bw/tw, bw*100/tw);
    println!("Frank: {} tokens ({}%)", r*fw/tw, fw*100/tw);
}
