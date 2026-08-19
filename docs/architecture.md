# Architecture

## Boundaries

The C# CLI owns user interaction and bridge process lifetime. It launches a
fresh Python process on an ephemeral loopback port and authenticates every
request with a random 256-bit bearer token inherited through the child
environment.

The Python bridge owns all pyatv state. It discovers Apple TVs with mDNS,
pairs the AirPlay service, persists credentials through pyatv's `FileStorage`,
and connects for playback or control operations.

Apple TV cannot fetch content from the PC's loopback address. For local files
and generated HLS, the bridge therefore starts a separate HTTP server on all
interfaces and advertises the PC's LAN address. The server uses an ephemeral
port, disables directory listings, and limits file-casting requests to the
selected filename.

## Crawl flow

1. The CLI starts the authenticated loopback bridge.
2. `pyatv.scan` discovers advertised services.
3. Pairing asks Apple TV for an AirPlay PIN and saves opaque credentials.
4. URL casting passes an HTTP(S) URL to `atv.stream.play_url`.
5. File casting exposes only the selected file over LAN HTTP, then passes that
   URL to Apple TV.
6. Status and stop reconnect using pyatv's stored credentials.

## Walk flow

1. FFmpeg uses Windows `gdigrab` to capture the virtual desktop.
2. It encodes H.264/yuv420p and writes a rolling HLS playlist.
3. The LAN HTTP server exposes the random temporary HLS directory.
4. pyatv asks Apple TV to play the playlist URL.
5. Ctrl+C terminates the bridge process tree, FFmpeg, and HTTP server.

## Why HTTP rather than a custom socket

HTTP keeps the language boundary inspectable and versionable while requiring
no third-party C# package. The bridge is loopback-only and authenticated.
Server-sent events or WebSockets can be added later if continuous telemetry is
needed; request/response is sufficient for the first vertical slices.

## Native implementation seam

The `/v1` bridge boundary intentionally hides pyatv. Future work can replace
discovery, pairing, session setup, timing, encryption, and transport behind the
same C# operations. Media capture and encoding remain independent of that
replacement.
