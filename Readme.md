# Installation & Running Guide

## Prerequisites

1. Install Rust and required toolchains:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup toolchain install stable nightly
rustup component add rust-src --toolchain nightly

```


2. Install `cargo-binstall`:
```bash
cargo install cargo-binstall

```


3. Install `bpf-linker`:
```bash
cargo binstall bpf-linker

```



## Build & Run

1. Build the project:
```bash
cargo build --release

```


2. Run the agent (requires root privileges to load eBPF programs):
```bash
sudo cargo run --release

```