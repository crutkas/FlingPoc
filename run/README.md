# Rust AirPlay foundation

`run/` is a Cargo workspace for the language-neutral AirPlay sidecar:

- `fling-airplay` is the reusable engine library.
- `fling-airplay-sidecar` exposes the authenticated localhost `/v1` contract.

## Current capability boundary

| Capability | Rust status |
| --- | --- |
| Health and capability API | Supported |
| mDNS AirPlay discovery | Supported |
| Credential model and per-user store | Building block only |
| Plain HTTP/RTSP and binary plist | Building block only |
| HAP pairing and pair verification | Python fallback |
| URL and local-file playback | Python fallback |
| Playback status and stop | Python fallback |
| FFmpeg-to-HLS desktop streaming | Python fallback |
| Native FairPlay mirroring | Unsupported |

The sidecar implements every existing route. Unported operations return HTTP
501 with the standard JSON error body. This permits contract testing without
misrepresenting protocol support.

## Build and test

```text
cargo build --release --manifest-path run/Cargo.toml
cargo test --manifest-path run/Cargo.toml --workspace
cargo clippy --manifest-path run/Cargo.toml --workspace --all-targets -- -D warnings
cargo fmt --manifest-path run/Cargo.toml --all -- --check
```

The C# CLI launches `run/target/release/fling-airplay-sidecar` when
`FLING_BRIDGE=rust`. `FLING_RUST_SIDECAR` can select another build.

## Security and provenance

The control listener binds only to loopback, compares the inherited bearer
credential in constant time, limits request bodies, and times out handlers.
RTSP builders reject request-line and header injection. Credential private key
material is intentionally excluded from `Debug`, and Unix stores are mode
`0600`.

This workspace does not contain native screen-mirroring or FairPlay code. See
`THIRD_PARTY_NOTICES.md` for protocol-reference attribution and direct
dependency licenses.

