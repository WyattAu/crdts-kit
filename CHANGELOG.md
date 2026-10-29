# Changelog

All notable changes are documented here.


All notable changes to this project are documented here. Format: [Keep a
Changelog](https://keepachangelog.com/): versions follow [semver](https://semver.org).

## [1.0.0] - 2026-10-29

### Declared stable

First 1.0. `crdts-kit` carries production collaboration state; this release
commits the RGA replicated string (tombstones, participant presence), the serde
and wasm surfaces as API-stable.

### Guarantees from here on

- Breaking changes to public items require a minor bump plus a `#[deprecated]`
  window of at least one minor release.
- MSRV bumps are minor releases, not patch releases.
- `#![deny(missing_docs)]` and `#![forbid(unsafe_code)]` are enforced.
- Documents mutated only by 1.x releases converge identically on every 1.x
  replica; convergence behaviour is contractual.

### Explicitly not guaranteed

- Serialized byte formats may gain optional fields in minors; readers must
  tolerate unknown keys.
- New CRDT types beyond the RGA string may arrive in minors.

## [0.1.0] - 2026-09-05

### Added
- Initial public release.
