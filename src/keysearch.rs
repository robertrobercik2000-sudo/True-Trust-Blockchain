//! Full keysearch implementation with X25519 + AES-GCM + TLV
#![forbid(unsafe_code)]
#![allow(dead_code)]

use crate::crypto_kmac::{kmac256_xof_fill, shake256_32};
use aes_gcm::{aead::{Aead, KeyInit}, Aes256Gcm, Nonce};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use x25519_dalek::{EphemeralSecret, PublicKey as X25519Public, StaticSecret};
use zeroize::{Zeroize, Zeroizing};

/* ------------------------ Constants ------------------------ */

/// Max enc_hint size (DoS guard)
pub const MAX_ENC_HINT_BYTES: usize = 32 * 1024; // 32 KiB

/* ------------------------ CT compare ------------------------ */

#[inline]
fn ct_eq32(a: &[u8; 32], b: &[u8; 32]) -> bool {
    let mut d = 0u8;
    for i in 0..32 { d |= a[i] ^ b[i]; }
    d == 0
}

#[inline]
#[allow(dead_code)]
fn ct_eq16(a: &[u8; 16], b: &[u8; 16]) -> bool {
    let mut d = 0u8;
    for i in 0..16 { d |= a[i] ^ b[i]; }
    d == 0
}

/* ----------------------------- TLV memo ----------------------------- */

pub mod tlv {
    use super::*;

    pub const T_ASCII: u8 = 0x01;
    pub const T_PROTOBUF: u8 = 0x02;
    pub const T_CIPHERTEXT_TO_SPEND: u8 = 0x03;
    pub const T_VALUE_PLAIN: u8 = 0x10;
    pub const T_VALUE_MASKED: u8 = 0x11;

    #[derive(Clone, Debug)]
    pub enum Item {
        Ascii(String),
        Protobuf(Vec<u8>),
        CiphertextToSpend(Vec<u8>),
        ValuePlain(u64),
        ValueMasked([u8; 8]),
        Raw(u8, Vec<u8>),
    }

    pub fn encode(items: &[Item]) -> Vec<u8> {
        let mut out = Vec::new();
        for it in items {
            match it {
                Item::Ascii(s) => push_tlv(&mut out, T_ASCII, s.as_bytes()),
                Item::Protobuf(b) => push_tlv(&mut out, T_PROTOBUF, b),
                Item::CiphertextToSpend(b) => push_tlv(&mut out, T_CIPHERTEXT_TO_SPEND, b),
                Item::ValuePlain(v) => { let le = v.to_le_bytes().to_vec(); push_tlv(&mut out, T_VALUE_PLAIN, &le); },
                Item::ValueMasked(b8) => push_tlv(&mut out, T_VALUE_MASKED, b8),
                Item::Raw(t, b) => push_tlv(&mut out, *t, b),
            }
        }
        out
    }

    fn push_tlv(out: &mut Vec<u8>, typ: u8, bytes: &[u8]) {
        out.push(typ);
        out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(bytes);
    }

    pub fn decode(mut data: &[u8]) -> Vec<Item> {
        let mut v = Vec::new();
        while data.len() >= 5 {
            let typ = data[0];
            let mut len_le = [0u8; 4]; len_le.copy_from_slice(&data[1..5]);
            let len = u32::from_le_bytes(len_le) as usize;
            data = &data[5..];
            if data.len() < len { break; }
            let body = &data[..len];
            let item = match typ {
                T_ASCII => Item::Ascii(String::from_utf8_lossy(body).into_owned()),
                T_PROTOBUF => Item::Protobuf(body.to_vec()),
                T_CIPHERTEXT_TO_SPEND => Item::CiphertextToSpend(body.to_vec()),
                T_VALUE_PLAIN if body.len()==8 => {
                    let mut b=[0u8;8]; b.copy_from_slice(body); Item::ValuePlain(u64::from_le_bytes(b))
                }
                T_VALUE_MASKED if body.len()==8 => { let mut b=[0u8;8]; b.copy_from_slice(body); Item::ValueMasked(b) }
                _ => Item::Raw(typ, body.to_vec()),
            };
            v.push(item);
            data = &data[len..];
        }
        v
    }
}

/* ----------------------------- Encoded payload ----------------------------- */

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HintPayloadV1 {
    pub r_blind: [u8; 32],
    pub value: u64,
    pub memo: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct DecodedHint {
    pub r_blind: [u8; 32],
    pub value: Option<u64>,
    pub memo_items: Vec<tlv::Item>,
}

/* ----------------------------- AAD and value modes ----------------------------- */

#[derive(Clone, Copy, Debug)]
pub enum AadMode { COutOnly, NetIdAndCOut(u32) }

#[derive(Clone, Copy, Debug)]
pub enum ValueConceal { None, Plain(u64), Masked(u64) }

#[inline]
fn aad_bytes(mode: AadMode, c_out: &[u8; 32]) -> [u8; 36] {
    match mode {
        AadMode::COutOnly => {
            let mut a = [0u8; 36]; a[4..].copy_from_slice(c_out); a
        }
        AadMode::NetIdAndCOut(net) => {
            let mut a = [0u8; 36]; a[..4].copy_from_slice(&net.to_le_bytes()); a[4..].copy_from_slice(c_out); a
        }
    }
}

#[inline]
fn value_mask(k_mat: &[u8], c_out: &[u8;32]) -> [u8;8] {
    let mut m = [0u8;8]; kmac256_xof_fill(k_mat, b"VALMASK", c_out, &mut m); m
}

/* ----------------------------- KeySearch Context ----------------------------- */

pub struct KeySearchCtx {
    vsk: StaticSecret,
}

impl KeySearchCtx {
    pub fn new(view_secret: [u8; 32]) -> Self { 
        Self { vsk: StaticSecret::from(view_secret) } 
    }

    /// Build full enc_hint (legacy variant)
    pub fn build_enc_hint(
        scan_pk: &X25519Public,
        c_out: &[u8; 32],
        payload: &HintPayloadV1,
    ) -> Vec<u8> {
        Self::build_enc_hint_ext(
            scan_pk, 
            c_out, 
            AadMode::COutOnly, 
            Some(payload.r_blind),
            if payload.value==0 { ValueConceal::None } else { ValueConceal::Plain(payload.value) },
            &tlv::decode(&payload.memo)
        )
    }

    /// Build enc_hint (production)
    pub fn build_enc_hint_ext(
        scan_pk: &X25519Public,
        c_out: &[u8; 32],
        aad_mode: AadMode,
        r_blind_opt: Option<[u8;32]>,
        val_mode: ValueConceal,
        memo_items: &[tlv::Item],
    ) -> Vec<u8> {
        let eph = EphemeralSecret::random_from_rng(OsRng);
        let eph_pub = X25519Public::from(&eph);
        let shared = eph.diffie_hellman(scan_pk);
        let k_mat = shared.as_bytes();

        let mut tag = [0u8; 32];
        kmac256_xof_fill(k_mat, b"HINT", c_out, &mut tag);
        let mut k_enc = [0u8; 32];
        kmac256_xof_fill(k_mat, b"ENC", c_out, &mut k_enc);
        let mut nonce12 = [0u8; 12];
        kmac256_xof_fill(k_mat, b"NONCE", c_out, &mut nonce12);

        let r_blind = r_blind_opt.unwrap_or_else(|| { let mut r=[0u8;32]; OsRng.fill_bytes(&mut r); r });

        let mut memo_vec = memo_items.to_vec();
        let mut legacy_value: u64 = 0;
        match val_mode {
            ValueConceal::None => {}
            ValueConceal::Plain(v) => { legacy_value = v; memo_vec.push(tlv::Item::ValuePlain(v)); }
            ValueConceal::Masked(v) => {
                let m = value_mask(k_mat, c_out);
                let masked = (v ^ u64::from_le_bytes(m)) .to_le_bytes();
                memo_vec.push(tlv::Item::ValueMasked(masked));
            }
        }
        let memo = tlv::encode(&memo_vec);

        let payload = HintPayloadV1 { r_blind, value: legacy_value, memo };

        let cipher = Aes256Gcm::new_from_slice(&k_enc).expect("aes key");
        let nonce = Nonce::from_slice(&nonce12);
        let aad = aad_bytes(aad_mode, c_out);
        let pt = bincode::serialize(&payload).expect("serialize payload");
        let ct = cipher.encrypt(nonce, aes_gcm::aead::Payload { msg: &pt, aad: &aad }).expect("encrypt");

        let mut out = Vec::with_capacity(32 + 32 + ct.len());
        out.extend_from_slice(eph_pub.as_bytes());
        out.extend_from_slice(&tag);
        out.extend_from_slice(&ct);

        Zeroizing::new(k_enc).zeroize();
        Zeroizing::new(nonce12).zeroize();
        out
    }

    /// Match and decrypt (legacy)
    pub fn try_match_and_decrypt(
        &self,
        c_out: &[u8; 32],
        enc_hint: &[u8],
    ) -> Option<([u8; 32], Option<HintPayloadV1>)> {
        self.try_match_and_decrypt_ext(c_out, enc_hint, AadMode::COutOnly)
            .map(|(k, dec)| (k, dec.map(|d| HintPayloadV1 { 
                r_blind: d.r_blind, 
                value: d.value.unwrap_or(0), 
                memo: tlv::encode(&d.memo_items) 
            })))
    }

    /// Match and decrypt (extended)
    pub fn try_match_and_decrypt_ext(
        &self,
        c_out: &[u8; 32],
        enc_hint: &[u8],
        aad_mode: AadMode,
    ) -> Option<([u8; 32], Option<DecodedHint>)> {
        if enc_hint.len() < 64 || enc_hint.len() > MAX_ENC_HINT_BYTES { return None; }

        let mut eph_pub32 = [0u8; 32];
        let mut tag32     = [0u8; 32];
        eph_pub32.copy_from_slice(&enc_hint[0..32]);
        tag32.copy_from_slice(&enc_hint[32..64]);

        let eph_pub = X25519Public::from(eph_pub32);
        let shared  = self.vsk.diffie_hellman(&eph_pub);
        let k_mat   = shared.as_bytes();

        let mut tag_calc = [0u8; 32];
        kmac256_xof_fill(k_mat, b"HINT", c_out, &mut tag_calc);
        let ok = ct_eq32(&tag_calc, &tag32);
        tag_calc.zeroize();
        if !ok { return None; }

        let mut k_search = [0u8; 32];
        kmac256_xof_fill(k_mat, b"KSEARCH", c_out, &mut k_search);

        if enc_hint.len() == 64 { return Some((k_search, None)); }

        let mut k_enc = [0u8; 32];
        kmac256_xof_fill(k_mat, b"ENC", c_out, &mut k_enc);
        let mut nonce12 = [0u8; 12];
        kmac256_xof_fill(k_mat, b"NONCE", c_out, &mut nonce12);
        let cipher = Aes256Gcm::new_from_slice(&k_enc).expect("aes key");
        let nonce = Nonce::from_slice(&nonce12);
        let ct = &enc_hint[64..];
        let aad = aad_bytes(aad_mode, c_out);

        let dec = match cipher.decrypt(nonce, aes_gcm::aead::Payload { msg: ct, aad: &aad }) {
            Ok(mut pt) => {
                let hp: HintPayloadV1 = match bincode::deserialize(&pt) { 
                    Ok(x) => x, 
                    Err(_) => { pt.zeroize(); return Some((k_search, None)); } 
                };
                pt.zeroize();
                let items = tlv::decode(&hp.memo);
                let mut val: Option<u64> = None;
                for it in &items {
                    match it {
                        tlv::Item::ValuePlain(v) => { val = Some(*v); break; }
                        tlv::Item::ValueMasked(mb) => {
                            let mask = value_mask(k_mat, c_out);
                            let x = u64::from_le_bytes(*mb) ^ u64::from_le_bytes(mask);
                            val = Some(x); break;
                        }
                        _ => {}
                    }
                }
                if val.is_none() && hp.value != 0 { val = Some(hp.value); }
                Some(DecodedHint { r_blind: hp.r_blind, value: val, memo_items: items })
            }
            Err(_) => None,
        };

        Zeroizing::new(k_enc).zeroize();
        Zeroizing::new(nonce12).zeroize();
        Some((k_search, dec))
    }

    /// Match only
    pub fn try_match(&self, c_out: &[u8; 32], enc_hint: &[u8]) -> Option<[u8; 32]> {
        self.try_match_and_decrypt_ext(c_out, enc_hint, AadMode::COutOnly).map(|(k, _)| k)
    }

    /// Stateless match
    pub fn try_match_stateless(
        &self,
        c_out: &[u8; 32],
        eph_pub: &[u8; 32],
        enc_hint_hash32: &[u8; 32],
    ) -> Option<[u8; 32]> {
        let eph = X25519Public::from(*eph_pub);
        let shared = self.vsk.diffie_hellman(&eph);
        let k_mat = shared.as_bytes();

        let mut tag = [0u8; 32];
        kmac256_xof_fill(k_mat, b"HINT", c_out, &mut tag);

        let mut enc = [0u8; 64];
        enc[..32].copy_from_slice(eph_pub);
        enc[32..].copy_from_slice(&tag);
        tag.zeroize();

        let h = shake256_32(&[&enc]);
        enc.zeroize();

        if &h != enc_hint_hash32 { return None; }

        let mut k = [0u8; 32];
        kmac256_xof_fill(k_mat, b"KSEARCH", c_out, &mut k);
        Some(k)
    }

    /// Batch scan
    pub fn scan<'a, I>(&self, outputs: I) -> Vec<FoundNote>
    where
        I: IntoIterator<Item = (&'a [u8; 32], &'a [u8])>,
    {
        let mut hits = Vec::new();
        for (idx, (c_out, enc_hint)) in outputs.into_iter().enumerate() {
            if let Some((k, _)) = self.try_match_and_decrypt_ext(c_out, enc_hint, AadMode::COutOnly) {
                hits.push(FoundNote { index: idx, c_out: *c_out, k_search: k });
            }
        }
        hits
    }

    /// Batch scan stateless
    pub fn scan_stateless<'a, I>(&self, outputs: I) -> Vec<FoundNote>
    where
        I: IntoIterator<Item = (&'a [u8; 32], &'a [u8; 32], &'a [u8; 32])>,
    {
        let mut hits = Vec::new();
        for (idx, (c_out, eph_pub, enc_hash)) in outputs.into_iter().enumerate() {
            if let Some(k) = self.try_match_stateless(c_out, eph_pub, enc_hash) {
                hits.push(FoundNote { index: idx, c_out: *c_out, k_search: k });
            }
        }
        hits
    }
}

#[derive(Clone, Debug)]
pub struct FoundNote {
    pub index: usize,
    pub c_out: [u8; 32],
    pub k_search: [u8; 32],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keysearch_roundtrip_full_hint_and_decrypt() {
        let mut view_secret = [0u8; 32]; OsRng.fill_bytes(&mut view_secret);
        let ctx = KeySearchCtx::new(view_secret);

        let mut c_out = [0u8; 32]; OsRng.fill_bytes(&mut c_out);

        let vsk = StaticSecret::from(view_secret);
        let vpub = X25519Public::from(&vsk);
        let enc_hint = KeySearchCtx::build_enc_hint_ext(
            &vpub,
            &c_out,
            AadMode::NetIdAndCOut(7),
            Some([7u8;32]),
            ValueConceal::Masked(42),
            &[tlv::Item::Ascii("hello".into())],
        );

        let (k, dec) = ctx.try_match_and_decrypt_ext(&c_out, &enc_hint, AadMode::NetIdAndCOut(7)).expect("should match");
        let d = dec.expect("payload");
        assert_eq!(d.value, Some(42));
        
        let eph_pub = { let mut e=[0u8;32]; e.copy_from_slice(&enc_hint[0..32]); X25519Public::from(e) };
        let shared  = StaticSecret::from(view_secret).diffie_hellman(&eph_pub);
        let mut ref_k = [0u8; 32]; kmac256_xof_fill(shared.as_bytes(), b"KSEARCH", &c_out, &mut ref_k);
        assert_eq!(k, ref_k);
    }
}
