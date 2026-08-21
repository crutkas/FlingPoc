# Implementation notes

## What is locked in

- Crawl and walk are vertical slices, not separate throwaway applications.
- C# owns the product surface and selected sidecar process lifetime.
- The language-neutral, authenticated `/v1` boundary is the compatibility seam.
- Python remains available while capabilities move to the reusable Rust engine.
- Device discovery is mDNS/Bonjour, not a broad network scan.
- Local control accepts traffic only from loopback and requires a per-launch
  random token.
- Apple TV media delivery is a separate LAN HTTP endpoint because the receiver
  must initiate the fetch.
- Native mirroring remains unsupported until FairPlay is separately licensed.

## Rust port sequence

1. Establish the workspace, reusable data model, capability reporting, safe
   RTSP/binary-plist primitives, credential-store model, and mDNS discovery.
2. Port HAP pair setup and pair verification from permissively licensed
   references, with attribution, protocol fixtures, and real-hardware
   validation.
3. Enable authenticated URL playback and playback session timing.
4. Port token-gated local-file serving, status, and stop.
5. Move the existing FFmpeg-to-HLS fallback behind the Rust engine.
6. Make Rust the default only after crawl and walk reach parity.

Each step preserves the `/v1` request and response shapes. Features stay
`pythonFallback` until they are complete; they do not silently downgrade.

## Compatibility choices

FFmpeg emits H.264 High Profile level 4.1 with yuv420p. This targets modern
Apple TV hardware and preserves desktop detail. If older receivers reject the
stream, the first fallback is Baseline Profile level 3.1 at 1280×720.

HLS uses one-second segments and a six-entry rolling playlist. Shorter segments
reduce latency but increase overhead and do not eliminate receiver buffering.

On Windows, the Python bridge selects `WindowsSelectorEventLoopPolicy` for
reliable zeroconf discovery. FFmpeg uses `subprocess.Popen` rather than asyncio
subprocess pipes because selector loops do not support Windows subprocess
pipes. The Rust mDNS implementation uses its own worker thread and must be
validated on VPN and virtual-adapter configurations.

## Provenance rules

The Rust dependency graph is limited to permissively licensed components. The
portable protocol behavior is informed by pyatv 0.18.0 under its MIT license;
the required notice is in `run/THIRD_PARTY_NOTICES.md`.

Do not add AirPlay sender or receiver implementations with GPL/LGPL lineage,
code derived from Apple's proprietary binaries, or unlicensed FairPlay
material. A clean dependency license is necessary but not sufficient: source
provenance must also be reviewable.

## Known limitations

- The Rust sidecar currently supports discovery, not pairing or playback.
- Desktop capture currently has no system audio.
- HLS is URL-based AirPlay playback, not low-latency screen mirroring.
- Hardware encoders, adaptive bitrate, window selection, and reconnect are not
  implemented.
- Real Apple TV pairing and playback require hardware validation.
- Local media/HLS must be permitted through the host firewall.
- The sidecar is process-scoped, so file casting and mirroring end when the CLI
  exits.
- URL casting also remains attached to the CLI because the AirPlay playback
  session must stay open until media completes.

## Next experiments

1. Validate Rust discovery against Apple TV HD and 4K generations.
2. Add HAP protocol fixtures derived only from permissively licensed sources.
3. Implement and hardware-test pair setup, credential persistence, and pair
   verification before enabling URL playback.
4. Measure 0.5-, 1-, and 2-second HLS segments and receiver buffer behavior.
5. Add WASAPI loopback audio and verify A/V synchronization.
6. Add Media Foundation or NVENC/Quick Sync encoders behind capability probes.
7. Obtain an official or independently licensed FairPlay implementation before
   scheduling native mirroring.

