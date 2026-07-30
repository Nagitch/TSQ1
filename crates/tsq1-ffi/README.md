# tsq1-ffi

`tsq1-ffi` builds the C-compatible dynamic library for TSQ1 conversion while
the core `tsq1` crate remains usable as a Rust `rlib` in `no_std` consumers.

The library exports:

- `tsq1_mid_to_tsq`
- `tsq1_buffer_free`

Build the dynamic library with:

```console
cargo build -p tsq1-ffi --release
```

The output keeps the `tsq1` library name used before the workspace split.
Callers must release each successful conversion buffer exactly once with
`tsq1_buffer_free`.
