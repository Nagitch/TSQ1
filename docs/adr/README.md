# Architecture Decision Records

This directory records decisions that shape the TSQ1 format or more than one
implementation surface. The English and Japanese v1 draft specifications
remain the format contract; ADRs explain why the contract and repository are
structured this way.

The initial records are retrospective. Their decision dates identify when the
decision first became established in the specification or merged
implementation. They were recorded as ADRs on 2026-08-18.

## Status lifecycle

- **Proposed**: under discussion and not yet an implementation contract.
- **Accepted**: the specification and implementations are expected to follow
  the decision.
- **Deprecated**: retained for history but no longer recommended for new work.
- **Superseded**: replaced by a linked later ADR.

Accepted records are immutable except for typo and link corrections. Replace a
decision by adding a new ADR and marking the old record superseded. New records
start from [`template.md`](template.md), use the next four-digit number, and
state binary-compatibility and cross-implementation effects explicitly.

## Index

| ADR | Status | Decision |
| --- | --- | --- |
| [0001](0001-represent-musical-and-absolute-time-explicitly.md) | Accepted | Represent musical and absolute time explicitly |
| [0002](0002-use-a-length-prefixed-extensible-chunk-container.md) | Accepted | Use a length-prefixed extensible chunk container |
| [0003](0003-edit-an-owned-model-and-encode-canonically.md) | Accepted | Edit an owned model and encode canonically |
| [0004](0004-treat-smf-as-checked-interoperability.md) | Accepted | Treat SMF as checked interoperability |
| [0005](0005-keep-the-core-no-std-and-integrations-separate.md) | Accepted | Keep the core `no_std` and integrations separate |
| [0006](0006-lock-rust-and-typescript-codecs-with-shared-fixtures.md) | Accepted | Lock Rust and TypeScript codecs with shared fixtures |
