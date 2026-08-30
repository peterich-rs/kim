## Summary
<!-- What does this PR do? 1-3 bullet points. -->

## Related issue
<!-- Link the issue. E.g. "Closes #42" or "Refs: PROJ-123" -->

## Changes
<!-- Brief description of what changed and why. -->

## Test plan
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `cd sdk/web && npm test` (if SDK / proto changed)
- [ ] `cd sdk/media && npm test` (if media worker changed)
- [ ] `cd sdk/mobile && dart format --output=none --set-exit-if-changed lib test hook && flutter analyze --fatal-infos --fatal-warnings && flutter test` (if mobile changed)
- [ ] New functionality manually tested
- [ ] Edge cases considered
