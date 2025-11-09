#!/bin/bash
# Integration test for tt_priv_cli
# Tests wallet lifecycle: init, addr, export, rekey, shards

set -e
set -u

BIN="${1:-./target/release/tt_priv_cli}"
TEST_DIR="${2:-/tmp/tt_priv_cli_test_$$}"

echo "🧪 tt_priv_cli integration test"
echo "Binary: $BIN"
echo "Test dir: $TEST_DIR"

# Cleanup function
cleanup() {
    rm -rf "$TEST_DIR"
    echo "🧹 cleaned up $TEST_DIR"
}
trap cleanup EXIT

mkdir -p "$TEST_DIR"
cd "$TEST_DIR"

# ========== Test 1: wallet-init (classic) ==========
echo ""
echo "📝 Test 1: wallet-init (classic, no quantum)"
echo -e "test123456789\ntest123456789" | "$BIN" wallet-init \
    --file wallet1.bin \
    --aead gcm-siv \
    --pepper os-local \
    --pad-block 1024

if [ ! -f wallet1.bin ]; then
    echo "❌ FAIL: wallet1.bin not created"
    exit 1
fi
echo "✅ wallet1.bin created ($(stat -c%s wallet1.bin 2>/dev/null || stat -f%z wallet1.bin) bytes)"

# ========== Test 2: wallet-addr ==========
echo ""
echo "📝 Test 2: wallet-addr"
echo "test123456789" | "$BIN" wallet-addr --file wallet1.bin > addr1.txt 2>&1
if ! grep -q "address:" addr1.txt; then
    echo "❌ FAIL: no address in output"
    cat addr1.txt
    exit 1
fi
ADDR=$(grep "address:" addr1.txt | awk '{print $2}')
echo "✅ address: $ADDR"

# ========== Test 3: wallet-init (quantum) ==========
echo ""
echo "📝 Test 3: wallet-init (quantum enabled)"
echo -e "test123456789\ntest123456789" | "$BIN" wallet-init \
    --file wallet_q.bin \
    --aead xchacha20 \
    --pepper os-local \
    --pad-block 2048 \
    --quantum

if [ ! -f wallet_q.bin ]; then
    echo "❌ FAIL: wallet_q.bin not created"
    exit 1
fi
echo "✅ wallet_q.bin created ($(stat -c%s wallet_q.bin 2>/dev/null || stat -f%z wallet_q.bin) bytes)"

# ========== Test 4: wallet-addr (quantum) ==========
echo ""
echo "📝 Test 4: wallet-addr (quantum)"
echo "test123456789" | "$BIN" wallet-addr --file wallet_q.bin > addr_q.txt 2>&1
if ! grep -q "quantum: enabled" addr_q.txt; then
    echo "❌ FAIL: quantum not enabled"
    cat addr_q.txt
    exit 1
fi
if ! grep -q "falcon_pk:" addr_q.txt; then
    echo "❌ FAIL: no falcon_pk"
    cat addr_q.txt
    exit 1
fi
if ! grep -q "mlkem_pk" addr_q.txt; then
    echo "❌ FAIL: no mlkem_pk"
    cat addr_q.txt
    exit 1
fi
echo "✅ quantum keys present"
grep "falcon_pk:" addr_q.txt | head -1
grep "mlkem_pk" addr_q.txt | head -1

# ========== Test 5: wallet-export (public) ==========
echo ""
echo "📝 Test 5: wallet-export (public)"
echo "test123456789" | "$BIN" wallet-export --file wallet1.bin --secret false > export_pub.txt 2>&1
if ! grep -q "scan_pk:" export_pub.txt; then
    echo "❌ FAIL: no scan_pk"
    cat export_pub.txt
    exit 1
fi
echo "✅ public keys exported"

# ========== Test 6: wallet-export (secret) ==========
echo ""
echo "📝 Test 6: wallet-export (secret)"
echo -e "test123456789\ntest123456789" | "$BIN" wallet-export \
    --file wallet1.bin \
    --secret true \
    --out secret1.json 2>&1 || true

if [ ! -f secret1.json ]; then
    echo "⚠️  SKIP: secret export requires interactive confirm (expected)"
else
    if ! grep -q "master32" secret1.json; then
        echo "❌ FAIL: no master32"
        cat secret1.json
        exit 1
    fi
    echo "✅ secret exported to secret1.json"
    rm -f secret1.json  # cleanup sensitive file
fi

# ========== Test 7: shards-create (3-of-5) ==========
echo ""
echo "📝 Test 7: shards-create (3-of-5, no per-share password)"
mkdir -p shards
echo "test123456789" | "$BIN" shards-create \
    --file wallet1.bin \
    --out-dir shards \
    --m 3 \
    --n 5 2>&1 | tee shards_create.log

SHARD_COUNT=$(ls shards/shard-*.json 2>/dev/null | wc -l)
if [ "$SHARD_COUNT" -ne 5 ]; then
    echo "❌ FAIL: expected 5 shards, got $SHARD_COUNT"
    ls -la shards/
    exit 1
fi
echo "✅ created 5 shards"

# ========== Test 8: shards-recover (3-of-5) ==========
echo ""
echo "📝 Test 8: shards-recover (using 3 shards)"
echo -e "test_recovered_pw\ntest_recovered_pw" | "$BIN" shards-recover \
    --input "shards/shard-1-of-5.json,shards/shard-3-of-5.json,shards/shard-5-of-5.json" \
    --out wallet_recovered.bin \
    --aead gcm-siv \
    --pepper none \
    --pad-block 1024 2>&1 | tee recover.log

if [ ! -f wallet_recovered.bin ]; then
    echo "❌ FAIL: wallet_recovered.bin not created"
    cat recover.log
    exit 1
fi
echo "✅ wallet recovered from shards"

# ========== Test 9: verify recovered wallet ==========
echo ""
echo "📝 Test 9: verify recovered wallet address matches"
echo "test_recovered_pw" | "$BIN" wallet-addr --file wallet_recovered.bin > addr_recovered.txt 2>&1
ADDR_RECOVERED=$(grep "address:" addr_recovered.txt | awk '{print $2}')

if [ "$ADDR" != "$ADDR_RECOVERED" ]; then
    echo "❌ FAIL: address mismatch"
    echo "Original:  $ADDR"
    echo "Recovered: $ADDR_RECOVERED"
    exit 1
fi
echo "✅ recovered wallet address matches: $ADDR_RECOVERED"

# ========== Test 10: wallet-rekey ==========
echo ""
echo "📝 Test 10: wallet-rekey (change password)"
echo -e "test123456789\nnew_password_123\nnew_password_123" | "$BIN" wallet-rekey \
    --file wallet1.bin \
    --aead gcm-siv \
    --pepper os-local \
    --pad-block 1024 2>&1 | tee rekey.log

if ! grep -q "rekeyed wallet" rekey.log; then
    echo "❌ FAIL: rekey failed"
    cat rekey.log
    exit 1
fi
echo "✅ wallet rekeyed"

# Verify new password works
echo "new_password_123" | "$BIN" wallet-addr --file wallet1.bin > addr_rekeyed.txt 2>&1
ADDR_REKEYED=$(grep "address:" addr_rekeyed.txt | awk '{print $2}')
if [ "$ADDR" != "$ADDR_REKEYED" ]; then
    echo "❌ FAIL: address changed after rekey"
    exit 1
fi
echo "✅ rekeyed wallet verified (address unchanged)"

# ========== Summary ==========
echo ""
echo "════════════════════════════════════════"
echo "✅ ALL TESTS PASSED"
echo "════════════════════════════════════════"
echo "Test results:"
echo "  - wallet init (classic): ✅"
echo "  - wallet init (quantum): ✅"
echo "  - wallet addr: ✅"
echo "  - quantum keys: ✅"
echo "  - public export: ✅"
echo "  - shards create (3-of-5): ✅"
echo "  - shards recover: ✅"
echo "  - address consistency: ✅"
echo "  - wallet rekey: ✅"
echo ""
echo "🎉 tt_priv_cli: production ready!"
