# Installation

Regulus is developed from source. Build and run it with Cargo.

## Requirements

- `git`
- Rust and Cargo (I recommended `rustup`)
- `pnpm` if you want to run the docs site

## Build the CLI

```sh
cargo build -p compiler_cli
```

This builds the `reggie` and `regulus` binaries. Run the CLI through Cargo
while developing:

```sh
cargo run -q -p compiler_cli -- --help
```

## Run checks

```sh
cargo fmt
cargo test
cargo clippy --workspace --all-targets
```

## Documentation Site

Run the docs site:

```sh
cd docs/website
pnpm install
pnpm docs:dev
```
