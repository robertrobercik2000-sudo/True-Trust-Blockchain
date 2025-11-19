# Building

## Requirements

- Rust 1.91.1+ (for Winterfell)
- Cargo

## Wallet CLI

```bash
cargo build --release --bin tt_priv_cli
./target/release/tt_priv_cli --help
```

## Node (Incomplete)

```bash
cargo build --release --bin tt_node
# Warning: No network layer, basic structure only
```

## Features

Default: `goldilocks`, `seeded_falcon`, `rand_chacha`, `winterfell_v2`

Optional:
- `randomx-ffi` - Requires RandomX C library
- `babybear` - 31-bit STARK field (educational only)

## RandomX (Optional)

```bash
git clone https://github.com/tevador/RandomX
cd RandomX && mkdir build && cd build
cmake .. && make && sudo make install
```

Then build with:
```bash
cargo build --features randomx-ffi
```

## Tests

```bash
cargo test --lib
```

Coverage: ~93% (unit tests only, no integration tests)
