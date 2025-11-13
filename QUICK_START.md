# 🚀 TT BLOCKCHAIN - QUICK START

## ⚡ Szybki Start - 3 kroki

### 1. Build
```bash
cargo build --release
```

### 2. Uruchom Wallet
```bash
# Utwórz portfel
export ALICE_PASS="my-secure-password-123"
./target/release/tt_priv_cli wallet-init --wallet-id alice --passphrase-env ALICE_PASS

# Pokaż adres
./target/release/tt_priv_cli wallet-addr --wallet-id alice --passphrase-env ALICE_PASS
```

### 3. Uruchom Node
```bash
# Start blockchain node
./target/release/tt_node start --data-dir ./node_data --listen 127.0.0.1:8333
```

---

## 📦 Co dostaniesz?

✅ **Wallet CLI** (tt_priv_cli)
- PQC: Falcon512 + Kyber768
- AEAD: AES-GCM-SIV / XChaCha20
- Shamir M-of-N shards

✅ **Blockchain Node** (tt_node)
- PoT consensus (RANDAO + trust)
- PoZS ZK proofs (Groth16)
- Bulletproofs (64-bit range)
- RISC0 zkVM (private tx)

---

## 📚 Więcej info

- `FINAL_INTEGRATION.md` - Pełna dokumentacja
- `README_NODE.md` - Node usage guide
- `INTEGRATION_SUMMARY.md` - Szczegóły integracji

---

**Wszystko działa! 5228 linii, 18 plików, 2 binaries!** 🎉
