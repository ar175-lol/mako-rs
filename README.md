# mako-rs

A lightweight Wayland notification daemon written in Rust.

## Installation

### To use `mako-rs` in your NixOS or Home Manager configuration, add this to your `flake.nix` inputs:

```nix
inputs.mako-rs.url = "github:ar175-lol/mako-rs";
```

### To use `mako-rs` in other distributions, first, ensure you have `cargo` and the required development libraries installed on your system.

#### 1. Install Dependencies
You need development packages for `wayland`, `cairo`, and `libxkbcommon`.

#### 2. Build and Install

Clone the repository and compile the binary in release mode:
```bash

git clone https://github.com/ar175-lol/mako-rs.git
cd mako-rs
cargo build --release
```
