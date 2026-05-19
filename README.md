# RZSTD

A utility that does a grep over ZST compressed files in parallel.

## Problem definition

The problem this tries to solve is that `zstdgrep` processes files sequentially. With this small tool, each file gets processed in a separate concurrent task.

## Usage

```sh
rzstd <regex> <file1> <file2> <file3>
```

## Example

```
rzstd 'ID = 1' ./file1.zst ./file2.zst ./file3.zst
```

## Building

### Debug build: 

```
cargo build 
```

### Release build:
```
cargo build --release
```

### Cross-compiling for Linux x86_64

`rzstd` has no C dependencies (zstd decoding is pure Rust via `ruzstd`), so a
fully static Linux binary can be built from any host — no Docker, gcc, clang,
or zig required:

```sh
# One-time: install the target and expose Rust's bundled linker on PATH
rustup target add x86_64-unknown-linux-musl
ln -sf "$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | sed -n 's/host: //p')/bin/rust-lld" ~/.cargo/bin/rust-lld

cargo build --release --target x86_64-unknown-linux-musl
```

The resulting `target/x86_64-unknown-linux-musl/release/rzstd` is a static
binary that runs on any Linux x86_64 machine. Linker settings live in
`.cargo/config.toml`.
