# Contributing to TSQ1

Thank you for improving TSQ1. Keep changes focused and preserve the distinction
between the draft format specification and the currently implemented subset.
Changes to an accepted format-level or cross-implementation decision must add
or supersede an [architecture decision record](docs/adr/README.md).

## Before opening a pull request

1. Open or identify a GitHub issue that describes the problem and intended
   outcome.
2. Create a branch from the latest `main`.
3. Update the English specification first when changing the format, then apply
   the equivalent change to the paired Japanese specification.
4. Update tests and public documentation with behavior changes.
5. Run the validation commands below.

## Local validation

The pinned toolchain in `rust-toolchain.toml` is installed automatically by
rustup.

```console
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo check -p tsq1-no-std-check
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
```

Use `cargo fmt --all` before committing when the format check reports changes.

## Pull requests

- Keep one coherent concern per pull request.
- Explain user or integrator impact and list exact verification commands.
- Link the issue with `Closes #<number>` when the pull request fully resolves
  it.
- Call out format compatibility or public API changes explicitly.
- Do not combine generated build output with source changes.

Maintainers may request a separate design issue before accepting changes to
the binary format.
