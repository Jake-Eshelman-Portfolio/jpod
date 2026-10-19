# Host Tests

Run these from `software/host_tests`:

```sh
cargo test
cargo html
```

- `cargo test` runs the host tests.
- `cargo html` tests the local Cargo alias, runs actor coverage, and opens the HTML report.
- The report is saved at `target/llvm-cov/html/index.html`.

Install coverage tools once if needed:

```sh
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov --locked
```