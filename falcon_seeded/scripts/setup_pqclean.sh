#!/bin/bash
# Setup PQClean Falcon-512 sources for falcon_seeded crate

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FALCON_DIR="$SCRIPT_DIR/../pqclean/crypto_sign/falcon-512"

echo "🔐 Setting up PQClean Falcon-512 sources..."

# Clone PQClean if not already present
if [ ! -d "$SCRIPT_DIR/../pqclean_tmp" ]; then
    echo "📥 Cloning PQClean repository..."
    git clone https://github.com/PQClean/PQClean.git "$SCRIPT_DIR/../pqclean_tmp"
fi

# Create target directory
mkdir -p "$FALCON_DIR"

# Copy Falcon-512 clean variant
echo "📋 Copying Falcon-512 sources..."
cp -r "$SCRIPT_DIR/../pqclean_tmp/crypto_sign/falcon-512/clean" "$FALCON_DIR/"

# Copy common/ for fips202.h and SHAKE3
echo "📋 Copying PQClean common files (fips202, SHAKE3)..."
cp -r "$SCRIPT_DIR/../pqclean_tmp/common" "$SCRIPT_DIR/../pqclean/"

# Cleanup
rm -rf "$SCRIPT_DIR/../pqclean_tmp"

echo "✅ Setup complete!"
echo ""
echo "PQClean Falcon-512 sources installed at:"
echo "  $FALCON_DIR/clean/"
echo ""
echo "You can now build falcon_seeded:"
echo "  cd $(dirname $SCRIPT_DIR)"
echo "  cargo build --release"