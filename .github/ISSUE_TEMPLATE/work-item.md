---
name: Work item
about: Define a scoped implementation, documentation, verification, or design task.
title: ""
labels: ""
assignees: ""
---

## Summary

<!-- What should change, in one or two sentences? -->

## Why

<!-- Link related specs, docs, PRs, tests, or observed gaps. -->

## Scope

-

## Out of scope

-

## Definition of done

- [ ] The requested change or decision is complete.
- [ ] Tests or verification evidence cover changed behavior when applicable.
- [ ] Public API changes include Rust documentation and examples.
- [ ] The README and both language versions of affected specifications agree.
- [ ] No known compatibility contradiction remains.

## Verification

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo test --workspace --all-features`

Additional verification:

-

## Expected files or areas

-

## Notes for Codex

- Prefer a focused pull request that closes this issue.
- Preserve existing behavior unless the issue explicitly changes it.
- Update English source documents first and paired Japanese documents second.
