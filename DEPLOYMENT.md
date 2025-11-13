# TRUE_TRUST Node Deployment Guide

## Quick Start (Single Node)

### 1. Build

```bash
git clone https://github.com/yourusername/tt-blockchain.git
cd tt-blockchain
cargo build --release --features zk-proofs
```

**Binary location:** `./target/release/tt-node`

### 2. Initialize Wallet

```bash
./target/release/tt-node init-wallet

# Output:
# Creating new post-quantum wallet...
# Enter password: ********
# Confirm password: ********
# ✓ Wallet created successfully!
#   Node ID: a3f8e9d2c1b4... (64 hex chars)
#   Location: /home/user/.tt_wallet
# ⚠️  Keep your password safe - it cannot be recovered!
```

### 3. Run Node

```bash
# Basic validator with 1M stake
./target/release/tt-node run --stake 1000000

# With custom config
./target/release/tt-node run \
  --wallet-dir ~/.my_wallet \
  --data-dir /mnt/blockchain \
  --listen 0.0.0.0:8000 \
  --stake 5000000
```

### 4. Check Node Info

```bash
./target/release/tt-node info

# Output:
# === TRUE_TRUST Node Info ===
# Node ID: a3f8e9d2c1b4...
# Wallet: /home/user/.tt_wallet
# 
# Post-Quantum Algorithms:
#   Signature: Falcon512
#   KEM: ML-KEM-768 (Kyber)
# 
# ZK Proofs: Enabled (Groth16/BN254)
```

---

## Multi-Node Testnet

### Network Setup

**3 Validator Nodes:**

**Node 1 (Bootstrap):**
```bash
# Terminal 1
./target/release/tt-node run \
  --wallet-dir ~/.wallet1 \
  --data-dir ./data1 \
  --listen 127.0.0.1:8001 \
  --stake 1000000
```

**Node 2:**
```bash
# Terminal 2
./target/release/tt-node run \
  --wallet-dir ~/.wallet2 \
  --data-dir ./data2 \
  --listen 127.0.0.1:8002 \
  --bootstrap 127.0.0.1:8001 \
  --stake 1500000
```

**Node 3:**
```bash
# Terminal 3
./target/release/tt-node run \
  --wallet-dir ~/.wallet3 \
  --data-dir ./data3 \
  --listen 127.0.0.1:8003 \
  --bootstrap 127.0.0.1:8001 \
  --stake 2000000
```

**Total Stake:** 4,500,000  
**Expected Leaders per Slot:** ~0.45 (45% chance at λ=10%)

---

## Production Deployment

### System Requirements

**Minimum (Light Validator):**
- CPU: 2 cores (x86_64)
- RAM: 4 GB
- Disk: 50 GB SSD
- Network: 10 Mbps

**Recommended (Full Validator):**
- CPU: 8 cores (x86_64, AVX2)
- RAM: 16 GB
- Disk: 500 GB NVMe SSD
- Network: 100 Mbps

**With ZK Proofs:**
- CPU: +2 cores for proving
- RAM: +4 GB for Groth16 circuit

### Security Hardening

#### 1. Firewall Rules

```bash
# Allow P2P
sudo ufw allow 8000/tcp

# Allow SSH (if needed)
sudo ufw allow 22/tcp

# Enable firewall
sudo ufw enable
```

#### 2. File Permissions

```bash
# Secure wallet directory
chmod 700 ~/.tt_wallet
chmod 600 ~/.tt_wallet/wallet.enc

# Secure data directory
chmod 700 ./data
```

#### 3. Systemd Service

Create `/etc/systemd/system/tt-node.service`:

```ini
[Unit]
Description=TRUE_TRUST Blockchain Node
After=network.target

[Service]
Type=simple
User=ttnode
Group=ttnode
WorkingDirectory=/opt/tt-node
ExecStart=/opt/tt-node/tt-node run \
  --wallet-dir /var/lib/tt-node/wallet \
  --data-dir /var/lib/tt-node/data \
  --listen 0.0.0.0:8000 \
  --stake 10000000
Restart=on-failure
RestartSec=10s
StandardOutput=journal
StandardError=journal
Environment="RUST_LOG=info"

# Security
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/tt-node

[Install]
WantedBy=multi-user.target
```

**Enable and start:**
```bash
sudo systemctl daemon-reload
sudo systemctl enable tt-node
sudo systemctl start tt-node

# Check status
sudo systemctl status tt-node

# View logs
sudo journalctl -u tt-node -f
```

#### 4. Monitoring

**Metrics (TODO):**
- Block height
- Peer count
- Trust score
- Leader slots won
- Memory/CPU usage

**Prometheus Exporter (Future):**
```bash
curl http://localhost:9090/metrics
```

---

## Backup & Recovery

### Wallet Backup

**Method 1: File Copy**
```bash
# Backup
cp -r ~/.tt_wallet ~/backup/wallet_$(date +%Y%m%d).tar.gz

# Restore
cp ~/backup/wallet_20250109.tar.gz ~/.tt_wallet
```

**Method 2: Shamir Secret Sharing (TODO)**
```bash
# Split wallet into 5 shares (need 3 to recover)
tt-node wallet export-shares --threshold 3 --total 5

# Distribute shares to:
#   1. USB drive
#   2. Paper backup
#   3. Cloud storage (encrypted)
#   4. Hardware key
#   5. Safe deposit box
```

### Blockchain Data Backup

```bash
# Stop node
sudo systemctl stop tt-node

# Backup database
tar -czf blockchain_$(date +%Y%m%d).tar.gz ./data/blockchain.db

# Restart node
sudo systemctl start tt-node
```

**Recovery:**
```bash
# Stop node
sudo systemctl stop tt-node

# Remove corrupted data
rm -rf ./data/blockchain.db

# Restore backup
tar -xzf blockchain_20250109.tar.gz -C ./data/

# Restart node (will sync from peers)
sudo systemctl start tt-node
```

---

## Network Configuration

### Port Forwarding (NAT)

**If behind router:**
```
Forward port 8000 TCP → <Your Node IP>:8000
```

**Verify:**
```bash
# From external machine
nc -zv <Your Public IP> 8000
```

### DNS Setup

**A Record:**
```
node1.yourblockchain.com → 203.0.113.10
```

**Usage:**
```bash
tt-node run --listen 0.0.0.0:8000 --bootstrap node1.yourblockchain.com:8000
```

---

## Performance Tuning

### Disk I/O

**SSD TRIM (Linux):**
```bash
sudo fstrim -v /
```

**Disable disk cache (if ramdisk):**
```bash
# Mount blockchain DB to tmpfs
sudo mount -t tmpfs -o size=4G tmpfs /var/lib/tt-node/cache
```

### Network Buffering

**Increase TCP buffer sizes:**
```bash
sudo sysctl -w net.core.rmem_max=16777216
sudo sysctl -w net.core.wmem_max=16777216
```

### CPU Affinity

**Pin node to specific cores:**
```bash
taskset -c 0-3 ./tt-node run --stake 1000000
```

---

## Troubleshooting

### Node Won't Start

**Error:** `Wallet not found`
```bash
# Solution: Initialize wallet first
tt-node init-wallet
```

**Error:** `Address already in use (port 8000)`
```bash
# Solution: Change port
tt-node run --listen 0.0.0.0:8001
```

**Error:** `Failed to open database`
```bash
# Solution: Remove corrupted DB
rm -rf ./data/blockchain.db
# Node will create fresh DB
```

### Node Not Syncing

**Check connectivity:**
```bash
# Ping bootstrap peer
nc -zv 127.0.0.1:8001

# Check logs
tail -f ./data/node.log
```

**Force resync:**
```bash
# Stop node
# Delete blockchain.db
# Restart node
```

### Low Leader Selection

**Symptoms:** Never winning slots despite high stake

**Check:**
1. Trust score (should be ~1.0 for honest node)
2. Stake amount (ensure > min_stake)
3. RANDAO participation (must commit + reveal)

**Debug:**
```bash
# Enable debug logging
RUST_LOG=debug tt-node run --stake 1000000
```

---

## Upgrade Guide

### Minor Version (e.g., 1.0.0 → 1.1.0)

```bash
# Stop node
sudo systemctl stop tt-node

# Backup data
tar -czf backup_$(date +%Y%m%d).tar.gz ./data

# Update binary
cargo build --release
sudo cp target/release/tt-node /opt/tt-node/

# Restart node
sudo systemctl start tt-node
```

### Major Version (e.g., 1.x → 2.0)

**May require data migration!**

```bash
# 1. Stop node
sudo systemctl stop tt-node

# 2. Backup EVERYTHING
tar -czf full_backup_$(date +%Y%m%d).tar.gz ~/.tt_wallet ./data

# 3. Run migration script
./migrate_v1_to_v2.sh

# 4. Update binary
sudo cp target/release/tt-node /opt/tt-node/

# 5. Restart
sudo systemctl start tt-node

# 6. Verify
tt-node info
```

---

## Support & Resources

**Documentation:**
- README.md - Quick start
- ARCHITECTURE.md - Technical deep dive
- DEPLOYMENT.md - This file

**Community:**
- GitHub: https://github.com/yourusername/tt-blockchain
- Discord: https://discord.gg/truetrustchain
- Forum: https://forum.truetrustchain.org

**Security Issues:**
- Email: security@truetrustchain.org
- PGP Key: [TODO]

---

## Checklist: Production Validator

- [ ] Hardware meets recommended specs
- [ ] Wallet initialized and backed up (multiple copies!)
- [ ] Systemd service configured
- [ ] Firewall rules applied
- [ ] Port forwarding configured (if NAT)
- [ ] DNS A record set (optional)
- [ ] Monitoring configured
- [ ] Stake deposited (min 100,000)
- [ ] Bootstrap peers configured
- [ ] Node synced to latest block
- [ ] RANDAO participation enabled
- [ ] Logs checked for errors
- [ ] Backup strategy documented

**Go live!** 🚀

---

**Security Reminder:**
- Never share your wallet password
- Keep multiple backups in different locations
- Monitor your node regularly
- Update promptly for security patches
