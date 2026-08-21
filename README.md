# FlingPoc

A Windows proof of concept for casting video and a desktop to Apple TV. C# owns
the product surface and process lifetime; an authenticated localhost sidecar
owns AirPlay protocol state.

```text
                     ┌→ Rust/fling-airplay foundation
C# CLI → local /v1 ──┤
                     └→ Python/pyatv crawl + HLS walk
                                             ↘ Apple TV
```

The **crawl** phase discovers, pairs, and casts URLs or local files. The
**walk** phase captures the Windows desktop with FFmpeg and serves a low-latency
HLS stream that Apple TV can play. HLS is the distributable fallback, not
native AirPlay mirroring.

## Prerequisites

- Windows 10 or newer with [Winget](https://learn.microsoft.com/windows/package-manager/winget/)
- [.NET 10 SDK](https://dotnet.microsoft.com/download)
- A Windows private network with the PC and Apple TV on the same subnet

The setup script installs Python 3.13, FFmpeg, and the stable Rust toolchain.
Allow the selected sidecar and FFmpeg through Windows Defender Firewall on
private networks. Discovery uses mDNS, and local media is served on an
ephemeral TCP port.

## Setup

```powershell
.\setup.ps1
.\.venv\Scripts\Activate.ps1
```

Setup installs the Python bridge dependencies, builds the Rust sidecar in
release mode, and builds the .NET solution.

## Select a sidecar

Python remains the default while crawl and walk are ported incrementally:

```powershell
$env:FLING_BRIDGE = "python"
$env:FLING_PYTHON = "$PWD\.venv\Scripts\python.exe"
```

Select the Rust foundation for discovery and API compatibility testing:

```powershell
$env:FLING_BRIDGE = "rust"
```

Set `FLING_RUST_SIDECAR` to override the Rust executable. Without an override,
the CLI searches `run\target\release` and then `run\target\debug`.

The Rust sidecar currently supports authenticated health checks, capability
reporting, and mDNS discovery. Pairing, playback, control, file serving, and
HLS mirroring return HTTP 501 with an instruction to select Python. This is
intentional: unfinished protocol work is not reported as supported.

```powershell
dotnet run --project src\Fling.Cli -- capabilities
```

## Crawl: discover, pair, and cast

Use the Python sidecar for the complete crawl slice:

```powershell
dotnet run --project src\Fling.Cli -- devices
dotnet run --project src\Fling.Cli -- pair <device-id>
dotnet run --project src\Fling.Cli -- cast-url <device-id> <https-url>
dotnet run --project src\Fling.Cli -- cast-file <device-id> C:\Videos\sample.mp4
dotnet run --project src\Fling.Cli -- status <device-id>
dotnet run --project src\Fling.Cli -- stop <device-id>
```

Pairing credentials are managed by the selected sidecar. Python uses pyatv's
per-user storage. The Rust library includes a per-user credential-store model
for the future HAP port, but does not write credentials until pairing is
enabled. `cast-file` stays running because Apple TV fetches the file from the
temporary LAN server.

## Walk: desktop streaming

Use the Python sidecar for the current HLS fallback:

```powershell
dotnet run --project src\Fling.Cli -- mirror <device-id>
```

Press Ctrl+C to stop FFmpeg and the media server. The current slice captures
the Windows virtual desktop. FFmpeg emits 30 fps H.264 with one-second
keyframes and a six-segment rolling HLS playlist. Apple TV may add its own
buffer, so this has several seconds of latency rather than interactive
mirroring.

Native AirPlay mirroring is explicitly unsupported. It requires Apple's
`/fp-setup` FairPlay exchange; no implementation will be added until an
official or independently licensed implementation is available.

## Validation

```powershell
dotnet test
py -m unittest discover -s bridge\tests -v
cargo test --manifest-path run\Cargo.toml --workspace
cargo clippy --manifest-path run\Cargo.toml --workspace --all-targets -- -D warnings
cargo fmt --manifest-path run\Cargo.toml --all -- --check
```

See [architecture](docs/architecture.md), [implementation notes](docs/implementation-notes.md),
and the [Rust workspace](run/README.md) for boundaries, limitations, and the
incremental port sequence.

