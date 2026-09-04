# tsq1

`tsq1` provides the complete owned TSQ1 v1 draft model, validation, canonical
binary encoding, checked musical/absolute time mapping, and Standard MIDI File
interoperability.

```rust
let midi = std::fs::read("input.mid")?;
let tsq = tsq1::convert_midi_to_tsq_vec(&midi)?;
let roundtrip_midi = tsq1::convert_tsq_to_midi_vec(&tsq)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Use `Sequence::decode`, edit its tracks, maps, anchors, markers, or unknown
chunks, then call `Sequence::encode` for lossless TSQ1 workflows. Enable the
optional `serde` feature for JSON-compatible serialization.

The default `std` feature enables `std::error::Error` integration and the C
ABI helpers. The companion
[`tsq1-ffi`](../tsq1-ffi/README.md) crate builds those helpers as a dynamic
library, while [`tsq1-osc`](../tsq1-osc/README.md) connects OSC events to
`osc-ir`. Disable the default feature to verify the allocation-backed
`no_std` Rust library:

```console
cargo check -p tsq1-no-std-check
```

The `calculation` module provides the version-pinned `openformula-kernel`
boundary for query/runtime consumers. It exposes typed scalar arguments,
standard function evaluation, compatibility metadata, and namespaced extension
registration while leaving timeline references and persistence in TSQ1. The
adapter remains available with `--no-default-features`.

The complete draft format is documented in the
[English specification](../../TSQ1_SPEC_v1.0_Draft.md) and
[Japanese specification](../../TSQ1_SPEC_v1.0_Draft_JA.md).
