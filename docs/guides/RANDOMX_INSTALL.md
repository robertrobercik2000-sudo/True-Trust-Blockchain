# 📦 RandomX Installation Guide

## ⚠️ WYMAGANE: Biblioteka RandomX (C)

Ten projekt **WYMAGA** oficjalnej biblioteki RandomX od tevador (używanej przez Monero).

Bez niej **build się NIE POWIEDZIE**.

---

## 🐧 Linux (Ubuntu/Debian)

### Metoda 1: Instalacja z kodu źródłowego (RECOMMENDED)

```bash
# 1. Zainstaluj zależności
sudo apt-get update
sudo apt-get install -y git build-essential cmake

# 2. Sklonuj RandomX
cd /tmp
git clone https://github.com/tevador/RandomX
cd RandomX

# 3. Build
mkdir build && cd build
cmake ..
make -j$(nproc)

# 4. Zainstaluj system-wide
sudo make install

# 5. Odśwież cache linkera
sudo ldconfig

# 6. Sprawdź instalację
ls -lh /usr/local/lib/librandomx.*
# Powinno pokazać: librandomx.so i librandomx.a
```

### Metoda 2: Build lokalny (bez sudo)

Jeśli nie masz uprawnień root:

```bash
# 1-3: jak wyżej (build)
cd /tmp
git clone https://github.com/tevador/RandomX
cd RandomX && mkdir build && cd build
cmake ..
make -j$(nproc)

# 4. NIE instaluj - ustaw zmienną środowiskową
export RANDOMX_LIB_DIR=/tmp/RandomX/build

# 5. Build projektu
cd /workspace
cargo build
```

---

## 🍎 macOS

```bash
# 1. Zainstaluj Homebrew (jeśli nie masz)
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# 2. Zainstaluj zależności
brew install cmake

# 3. Sklonuj i zbuduj RandomX
cd /tmp
git clone https://github.com/tevador/RandomX
cd RandomX && mkdir build && cd build
cmake ..
make -j$(sysctl -n hw.ncpu)

# 4. Zainstaluj
sudo make install

# 5. Sprawdź
ls -lh /usr/local/lib/librandomx.*
```

---

## 🪟 Windows

### Opcja A: MSYS2 (RECOMMENDED)

```bash
# 1. Zainstaluj MSYS2: https://www.msys2.org/

# 2. W terminalu MSYS2:
pacman -S git mingw-w64-x86_64-gcc mingw-w64-x86_64-cmake make

# 3. Sklonuj RandomX
cd /tmp
git clone https://github.com/tevador/RandomX
cd RandomX && mkdir build && cd build

# 4. Build
cmake .. -G "Unix Makefiles"
make -j$(nproc)

# 5. Skopiuj bibliotekę
cp librandomx.dll C:/Windows/System32/
```

### Opcja B: Visual Studio

```powershell
# 1. Zainstaluj Visual Studio 2022 (Community) z C++ workload

# 2. Otwórz Developer PowerShell

# 3. Sklonuj RandomX
git clone https://github.com/tevador/RandomX
cd RandomX
mkdir build
cd build

# 4. Build
cmake .. -G "Visual Studio 17 2022" -A x64
cmake --build . --config Release

# 5. Skopiuj
copy Release\randomx.dll C:\Windows\System32\
copy Release\randomx.lib C:\Program Files\
```

---

## 🧪 Weryfikacja instalacji

### Test 1: Linkowanie

```bash
cd /workspace
cargo build --lib
```

**Sukces jeśli**:
```
Finished `dev` profile [unoptimized + debuginfo] target(s) in X.XXs
```

**Błąd jeśli**:
```
= note: ld: library not found for -lrandomx
```
→ Biblioteka nie jest zainstalowana lub nie jest w ścieżce.

---

### Test 2: Runtime

```bash
cd /workspace
cargo test --lib pow_randomx_monero::tests::test_hash_deterministic -- --ignored
```

**Sukces jeśli**:
```
test pow_randomx_monero::tests::test_hash_deterministic ... ok
```

---

## 🔧 Troubleshooting

### Problem: `cannot find -lrandomx`

**Rozwiązanie 1**: Sprawdź czy biblioteka istnieje
```bash
sudo find / -name "librandomx.*" 2>/dev/null
```

**Rozwiązanie 2**: Ustaw `RANDOMX_LIB_DIR`
```bash
export RANDOMX_LIB_DIR=/path/to/RandomX/build
cargo clean && cargo build
```

**Rozwiązanie 3**: Użyj static linking
```bash
export RANDOMX_STATIC=1
cargo clean && cargo build
```

---

### Problem: `librandomx.so: cannot open shared object file`

**Rozwiązanie** (Linux):
```bash
# Dodaj /usr/local/lib do LD_LIBRARY_PATH
echo 'export LD_LIBRARY_PATH=/usr/local/lib:$LD_LIBRARY_PATH' >> ~/.bashrc
source ~/.bashrc

# LUB odśwież ldconfig
sudo ldconfig
```

---

### Problem: Linker znajduje bibliotekę, ale runtime wywala się

**Możliwe przyczyny**:
1. **Wersja biblioteki**: Sprawdź czy masz najnowszą wersję RandomX
   ```bash
   cd /tmp/RandomX
   git pull
   cd build && cmake .. && make && sudo make install
   ```

2. **ABI incompatibility**: Przebuduj projekt od zera
   ```bash
   cargo clean
   rm -rf target/
   cargo build
   ```

---

### Problem: Build działa, ale testy failują z `VmCreateFailed`

**Przyczyna**: CPU nie wspiera wymaganych instrukcji (AES-NI, AVX2).

**Rozwiązanie**: Sprawdź flagi CPU
```bash
# Linux
grep -E 'aes|avx' /proc/cpuinfo

# macOS
sysctl -a | grep machdep.cpu.features
```

Jeśli brak AES-NI lub AVX2 → RandomX będzie **bardzo wolny** (bez JIT).

---

## 📊 Performance po instalacji

### Oczekiwane wyniki (single-core):

| CPU | Hash/s | Note |
|-----|--------|------|
| Intel i5-12600K | ~4000 | AES-NI + AVX2 + JIT |
| AMD Ryzen 5 5600X | ~5000 | Najlepsza performance |
| ARM (M1/M2) | ~2000 | Brak natywnego JIT |
| Stare CPU (bez AES) | ~200 | Fallback interpreter |

---

## 🚀 Quick Start (po instalacji)

```bash
# Build projektu
cd /workspace
cargo build --release

# Run node
./target/release/tt_node --help

# Test mining (wymaga wallet)
cargo run --bin tt_wallet -- init
cargo run --bin tt_node -- start
```

---

## 📚 Dodatkowe zasoby

- **RandomX repo**: https://github.com/tevador/RandomX
- **RandomX spec**: https://github.com/tevador/RandomX/blob/master/doc/specs.md
- **Monero integration**: https://github.com/monero-project/monero/tree/master/external/randomx
- **Performance tips**: https://github.com/tevador/RandomX#performance

---

## ✅ Checklist instalacji

- [ ] ✅ Zainstalowane zależności (git, cmake, gcc)
- [ ] ✅ Sklonowany RandomX z GitHub
- [ ] ✅ Zbudowany RandomX (`make` sukces)
- [ ] ✅ Zainstalowany system-wide (`sudo make install`)
- [ ] ✅ Odświeżony ldconfig (`sudo ldconfig`)
- [ ] ✅ `cargo build` działa bez błędów linkera
- [ ] ✅ Testy przechodzą (`cargo test`)

---

**Status po instalacji**: 🚀 **READY TO MINE!**
