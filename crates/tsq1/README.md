# tsq1

`tsq1` converts Standard MIDI File bytes to and from the musical-time subset
of the TSQ1 binary timeline format.

```rust
let midi = std::fs::read("input.mid")?;
let tsq = tsq1::convert_midi_to_tsq_vec(&midi)?;
let roundtrip_midi = tsq1::convert_tsq_to_midi_vec(&tsq)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

The default `std` feature enables `std::error::Error` integration and the C
ABI helpers. The companion
[`tsq1-ffi`](../tsq1-ffi/README.md) crate builds those helpers as a dynamic
library. Disable the default feature to verify the allocation-backed `no_std`
Rust library:

```console
cargo check -p tsq1-no-std-check
```

The implemented subset and known limitations are maintained in the
[repository README](../../README.md#implementation-status). The complete
draft format is documented in the
[English specification](../../TSQ1_SPEC_v1.0_Draft.md) and
[Japanese specification](../../TSQ1_SPEC_v1.0_Draft_JA.md).
