# tsq1-osc

`tsq1-osc` connects TSQ1 OSC events to the experimental
[`osc-ir`](https://crates.io/crates/osc-ir) model.

It provides:

- MessagePack conversion between `osc_ir::IrValue` and TSQ1 events
- nested OSC bundle preservation
- validation and construction of byte-accurate RAW OSC events

```rust
use osc_ir::IrValue;
use tsq1::TimeDomain;

let event = tsq1_osc::event_from_ir(
    TimeDomain::Musical,
    0,
    &IrValue::from("hello"),
)?;
let decoded = tsq1_osc::event_to_ir(&event)?;
assert_eq!(decoded, IrValue::from("hello"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

The core `tsq1` codec continues to work with `no_std + alloc`; this integration
crate owns the standard-library-dependent MessagePack codec.
