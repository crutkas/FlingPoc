# Architecture

## Boundaries

The C# CLI owns commands, user interaction, and sidecar process lifetime. It
launches a fresh process on an ephemeral loopback port and authenticates every
request with a random 256-bit bearer token inherited through the child
environment.

`FLING_BRIDGE` selects `python` (the default) or `rust`. Both sidecars speak the
same JSON `/v1` contract, so C# remains independent of AirPlay protocol
implementation details. Process isolation also prevents protocol or media
failures from corrupting product state.

The Python sidecar owns the currently complete crawl and walk slices. pyatv
discovers Apple TVs with mDNS, pairs the AirPlay service, persists credentials,
and connects for playback or control operations. FFmpeg and the token-gated LAN
HTTP server provide file playback and HLS desktop streaming.

The Rust sidecar is the replacement seam. Its `fling-airplay` library currently
contains:

- cross-platform `_airplay._tcp.local.` mDNS discovery;
- `/v1` device, playback, pairing, error, and capability models;
- a per-user HAP credential-store model that avoids debug-printing private key material;
- bounded plain HTTP/RTSP framing with header-injection checks; and
- binary-plist serialization and parsing.

HAP pair setup, pair verification, encrypted sessions, URL playback, status,
stop, LAN media serving, and HLS capture remain capability-gated. Unsupported
Rust routes return JSON errors rather than pretending to be complete.

## Local control contract

The sidecars bind only to `127.0.0.1`. Requests are limited to 1 MiB, have a
15-second processing timeout, and require the launch token. Responses use the
existing `{"error":"message"}` shape. `/v1/capabilities` is additive and lets
hosts inspect the current implementation boundary.

The stable routes are:

- `GET /health`
- `GET /v1/capabilities`
- `GET /v1/devices`
- `POST /v1/pair/start`
- `POST /v1/pair/finish`
- `POST /v1/play`
- `POST /v1/file`
- `POST /v1/mirror`
- `POST /v1/status`
- `POST /v1/stop`

## Crawl flow

The complete Python flow is:

1. The CLI starts the authenticated loopback sidecar.
2. `pyatv.scan` discovers advertised services.
3. Pairing asks Apple TV for an AirPlay PIN and saves opaque credentials.
4. URL casting passes an HTTP(S) URL to `atv.stream.play_url`.
5. File casting exposes only the selected file over LAN HTTP, then passes that
   URL to Apple TV.
6. Status and stop reconnect using pyatv's stored credentials.

The first Rust increment implements steps 1 and 2. Later increments replace one
capability at a time while the Python path remains available for comparison.

## Walk flow

The distributable fallback remains on Python:

1. FFmpeg uses Windows `gdigrab` to capture the virtual desktop.
2. It encodes H.264/yuv420p and writes a rolling HLS playlist.
3. The LAN HTTP server exposes the random temporary HLS directory.
4. pyatv asks Apple TV to play the playlist URL.
5. Ctrl+C terminates the sidecar process tree, FFmpeg, and HTTP server.

Apple TV cannot fetch content from the PC's loopback address. LAN media
therefore uses a separate server on all interfaces with an ephemeral port,
disabled directory listings, an allowlist, and a random per-process path
token.

## Why a sidecar

HTTP keeps the language boundary inspectable, authenticated, versionable, and
usable by hosts other than C#. A sidecar avoids exposing a large unsafe FFI
surface through C# P/Invoke and preserves crash isolation.

## Native mirroring boundary

URL playback and HLS do not implement the proprietary low-latency
screen-mirroring transport. Native mirroring additionally requires H.264/RTP
transport and Apple's `/fp-setup` FairPlay exchange. The capability remains
explicitly `unsupported` until an official Apple license or an independently
licensed implementation is available. GPL/LGPL or proprietary-derived sender
code is not an acceptable dependency.

