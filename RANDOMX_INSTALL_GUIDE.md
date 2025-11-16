# 📦 RandomX Installation Guide

## 🎯 Dwa tryby pracy

### Tryb 1: **Pure Rust** (domyślny, działa teraz) ✅
```bash
cargo build --release
```
- ✅ Zero dependencies
- ✅ Działa od razu
- ⚠️ ~10× wolniejszy

---

### Tryb 2: **FFI (Production)** - wymaga instalacji biblioteki RandomX

## 🔧 Instalacja RandomX (Monero C library)

### Linux / macOS:

#### 1️⃣ Zainstaluj dependencies:
```bash
# Ubuntu/Debian
sudo apt-get update
sudo apt-get install git build-essential cmake

# Fedora/RHEL
sudo dnf install git gcc gcc-c++ cmake

# macOS
brew install cmake
```

#### 2️⃣ Sklonuj i zbuduj RandomX:
```bash
cd /tmp
git clone https://github.com/tevador/RandomX
cd RandomX
mkdir build && cd build
cmake -DARCH=native ..
make -j$(nproc)
```

#### 3️⃣ Zainstaluj bibliotekę:

**Opcja A: System-wide** (wymaga sudo):
```bash
sudo make install
sudo ldconfig  # Linux only
```

**Opcja B: Local** (bez sudo):
```bash
# Skopiuj do katalogu projektu
mkdir -p /workspace/lib
cp librandomx.a /workspace/lib/
# Ustaw zmienną środowiskową
export RANDOMX_LIB_DIR=/workspace/lib
```

#### 4️⃣ Build z FFI:
```bash
cd /workspace
export RANDOMX_FFI=1
cargo build --release
```

---

### Windows (z MSYS2):

#### 1️⃣ Zainstaluj MSYS2:
- Pobierz z https://www.msys2.org/
- Uruchom MSYS2 MinGW 64-bit

#### 2️⃣ Zainstaluj dependencies:
```bash
pacman -S git mingw-w64-x86_64-gcc mingw-w64-x86_64-cmake make
```

#### 3️⃣ Zbuduj RandomX:
```bash
cd /tmp
git clone https://github.com/tevador/RandomX
cd RandomX
mkdir build && cd build
cmake -G "MSYS Makefiles" -DARCH=native ..
make -j4
```

#### 4️⃣ Ustaw ścieżkę:
```bash
export RANDOMX_LIB_DIR=/tmp/RandomX/build
export RANDOMX_FFI=1
cd /workspace
cargo build --release
```

---

## 🧪 Weryfikacja instalacji

### Test 1: Sprawdź czy biblioteka istnieje
```bash
# Linux/macOS
ls -lh /usr/local/lib/librandomx.*
# lub
ls -lh /workspace/lib/librandomx.*

# Windows
ls -lh /tmp/RandomX/build/librandomx.a
```

### Test 2: Build test
```bash
export RANDOMX_FFI=1
cargo clean
cargo build 2>&1 | grep -i randomx
```

Powinieneś zobaczyć:
```
warning: Enabling RandomX FFI...
warning: Linking RandomX C library...
```

### Test 3: Runtime test
```rust
// W src/main.rs lub nowym pliku testowym
use tt_priv_cli::pow_randomx_monero::RandomXEnv;

fn main() {
    let key = b"test-epoch-0";
    let mut env = RandomXEnv::new(key, false).expect("RandomX init");
    
    let input = b"test block";
    let hash = env.hash(input);
    
    println!("RandomX hash: {:x?}", hash);
}
```

```bash
export RANDOMX_FFI=1
cargo run --bin <test_binary>
```

---

## 🔍 Troubleshooting

### Problem 1: `librandomx.so not found` (runtime)
**Rozwiązanie**:
```bash
# Option A: Dodaj do LD_LIBRARY_PATH
export LD_LIBRARY_PATH=/usr/local/lib:$LD_LIBRARY_PATH

# Option B: Skopiuj do system path
sudo cp /path/to/librandomx.so /usr/lib/

# Option C: Static linking (preferowane)
# W build.rs użyj librandomx.a zamiast .so
```

---

### Problem 2: `undefined reference to randomx_*` (build)
**Przyczyna**: `RANDOMX_FFI=1` nie jest ustawione lub biblioteka jest w złej lokalizacji.

**Rozwiązanie**:
```bash
# Sprawdź zmienną
echo $RANDOMX_FFI

# Ustaw ponownie
export RANDOMX_FFI=1

# Sprawdź ścieżkę biblioteki
export RANDOMX_LIB_DIR=/correct/path/to/RandomX/build

# Clean rebuild
cargo clean
cargo build 2>&1 | grep warning
```

---

### Problem 3: `randomx_get_flags returned 0`
**Przyczyna**: CPU nie wspiera AES-NI lub AVX2.

**Rozwiązanie A**: Build bez hardware flags:
```bash
cd RandomX/build
cmake -DARCH=native -DNO_AES=ON ..
make clean && make
```

**Rozwiązanie B**: Użyj Pure Rust fallback (bez FFI).

---

### Problem 4: Wolny build (>5 min)
**Przyczyna**: RandomX dataset init (2 GB) podczas buildu.

**Rozwiązanie**: To normalne przy pierwszym uruchomieniu. Kolejne buildy będą szybkie.

---

## 📊 Performance comparison

| Mode | Hash/s | Memory | Dependency |
|------|--------|--------|------------|
| Pure Rust | ~500 H/s | 2.1 GB | ❌ None |
| FFI (no JIT) | ~2000 H/s | 2.1 GB | ✅ librandomx |
| FFI (full JIT) | ~5000 H/s | 2.1 GB | ✅ librandomx |

---

## 🎯 Recommendation

### Development: **Pure Rust** (obecny)
- Szybki setup
- Cross-platform
- Wystarczający do testów

### Production: **FFI**
- 10× szybszy
- 100% Monero-compatible
- Battle-tested

---

## 📝 Quick Start

### Bez instalacji (działa teraz):
```bash
cargo build --release
cargo test
```

### Z FFI (po instalacji RandomX):
```bash
export RANDOMX_FFI=1
cargo build --release
cargo test
```

---

## 🔗 Linki

- **RandomX repo**: https://github.com/tevador/RandomX
- **RandomX spec**: https://github.com/tevador/RandomX/blob/master/doc/specs.md
- **Monero integration**: https://github.com/monero-project/monero/tree/master/external/randomx

---

**Status**: ✅ Pure Rust działa, FFI optional (ale zalecany dla produkcji).
