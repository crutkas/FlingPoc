# Implementation notes

## What is locked in

- Crawl and walk are vertical slices, not separate throwaway applications.
- C# owns the product surface; Python is an explicitly replaceable protocol
  adapter.
- Device discovery is mDNS/Bonjour, not a broad network scan.
- Local control accepts traffic only from loopback and requires a per-launch
  random token.
- Apple TV media delivery is a separate LAN HTTP endpoint because the receiver
  must initiate the fetch.
- pyatv owns credential persistence so opaque protocol credentials do not pass
  through or get stored by the C# layer.

## Compatibility choices

FFmpeg emits H.264 High Profile level 4.1 with yuv420p. This targets modern
Apple TV hardware and preserves desktop detail. If older receivers reject the
stream, the first fallback is Baseline Profile level 3.1 at 1280×720.

HLS uses one-second segments and a six-entry rolling playlist. Shorter segments
reduce latency but increase overhead and do not eliminate receiver buffering.

On Windows, the bridge selects `WindowsSelectorEventLoopPolicy` for reliable
zeroconf discovery. FFmpeg uses `subprocess.Popen` rather than asyncio
subprocess pipes because selector loops do not support Windows subprocess
pipes.

## Known limitations

- Desktop capture currently has no system audio.
- This pipeline is URL-based AirPlay playback, not the proprietary low-latency
  screen-mirroring transport.
- Hardware encoders, adaptive bitrate, window selection, and reconnect are not
  implemented.
- Real Apple TV pairing and playback require hardware validation.
- Local media/HLS must be permitted through the host firewall.
- The bridge is process-scoped, so file casting and mirroring end when the CLI
  exits.
- URL casting also remains attached to the CLI because pyatv keeps the AirPlay
  playback session open until media completes.

## Next experiments

1. Measure capture-to-display latency across Apple TV HD and 4K generations.
2. Compare 0.5-, 1-, and 2-second HLS segments and receiver buffer behavior.
3. Add WASAPI loopback audio and verify A/V synchronization.
4. Add Media Foundation or NVENC/Quick Sync encoders behind capability probes.
5. Capture only traffic between this client and an authorized test Apple TV to
   document mirroring session setup, timing, encryption, and recovery.
6. Replace Python components incrementally while preserving the `/v1` contract.
