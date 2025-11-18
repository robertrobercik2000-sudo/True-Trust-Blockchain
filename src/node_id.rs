#![forbid(unsafe_code)]

//! Node identity helpers (PQ-only).
//!
//! NodeId = SHAKE256("TT-NODE-ID.FALCON.v1" || falcon_pk_bytes)[0..32]
//!
//! Używane wszędzie jako identyfikator walidatora (RTT, konsensus, P2P).

use pqcrypto_falcon::falcon512;
use pqcrypto_traits::sign::PublicKey as PQPublicKey;
use sha3::{
    digest::{ExtendableOutput, Update, XofReader},
    Shake256,
};

/// Globalny identyfikator noda używany przez RTT / konsensus / P2P.
///
/// To NIE jest klucz publiczny, tylko 32-bajtowy fingerprint.
pub type NodeId = [u8; 32];

/// Wylicza NodeId z klucza publicznego Falcon512.
pub fn node_id_from_falcon_pk(pk: &falcon512::PublicKey) -> NodeId {
    let mut h = Shake256::default();
    h.update(b"TT-NODE-ID.FALCON.v1");
    h.update(pk.as_bytes());
    let mut out = [0u8; 32];
    h.finalize_xof().read(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use pqcrypto_falcon::falcon512;

    #[test]
    fn node_id_is_stable_for_same_pk() {
        let (pk, _sk) = falcon512::keypair();
        let id1 = node_id_from_falcon_pk(&pk);
        let id2 = node_id_from_falcon_pk(&pk);
        assert_eq!(id1, id2);
    }

    #[test]
    fn node_id_differs_for_different_keys() {
        let (pk1, _sk1) = falcon512::keypair();
        let (pk2, _sk2) = falcon512::keypair();
        let id1 = node_id_from_falcon_pk(&pk1);
        let id2 = node_id_from_falcon_pk(&pk2);
        assert_ne!(id1, id2);
    }
}
