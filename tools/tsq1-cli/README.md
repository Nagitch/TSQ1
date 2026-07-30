# tsq1-cli

`tsq1-cli` converts files between Standard MIDI File and TSQ1 encodings, emits
the complete model as JSON, and validates binary structure with byte offsets.

```console
# input.mid -> input.tsq
cargo run -p tsq1-cli -- input.mid

# input.tsq -> input.mid
cargo run -p tsq1-cli -- input.tsq --direction tsq-to-midi

# equivalent explicit conversion subcommand
cargo run -p tsq1-cli -- convert input.mid

# choose an output path
cargo run -p tsq1-cli -- input.mid --output output/example.tsq

# inspect every TSQ1 section as JSON
cargo run -p tsq1-cli -- inspect input.tsq

# validate without rewriting
cargo run -p tsq1-cli -- validate input.tsq
```

The command never overwrites the input path by default; it replaces the file
extension to derive the output path. Use `--help` for all options.

See the [repository README](../../README.md) for format status and development
commands.
