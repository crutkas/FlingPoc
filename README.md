# FlingPoc

A Windows proof of concept for casting video and a desktop to Apple TV:

```text
C# CLI → authenticated localhost HTTP → Python/pyatv → Apple TV
                                      ↘ FFmpeg → HLS → LAN HTTP ↗
```

The **crawl** phase discovers, pairs, and casts URLs or local files. The
**walk** phase captures the Windows desktop with FFmpeg and serves a low-latency
HLS stream that Apple TV can play. This is not yet native AirPlay mirroring;
that protocol research belongs to the run phase.

## Prerequisites

- Windows 10 or newer
- [.NET 10 SDK](https://dotnet.microsoft.com/download)
- Python 3.11 or newer
- [FFmpeg](https://ffmpeg.org/download.html) on `PATH` for desktop capture
- A Windows private network with the PC and Apple TV on the same subnet

Allow Python and FFmpeg through Windows Defender Firewall on private networks.
Discovery uses mDNS, and local media is served on an ephemeral TCP port.

## Setup

```powershell
py -m venv .venv
.\.venv\Scripts\Activate.ps1
py -m pip install -r bridge\requirements.txt
dotnet build
```

The CLI starts and stops the Python bridge automatically. Set `FLING_PYTHON` to
the virtual environment's Python executable if `python` does not resolve to it.

```powershell
$env:FLING_PYTHON = "$PWD\.venv\Scripts\python.exe"
```

## Crawl: discover, pair, and cast

```powershell
dotnet run --project src\Fling.Cli -- devices
dotnet run --project src\Fling.Cli -- pair <device-id>
dotnet run --project src\Fling.Cli -- cast-url <device-id> <https-url>
dotnet run --project src\Fling.Cli -- cast-file <device-id> C:\Videos\sample.mp4
dotnet run --project src\Fling.Cli -- status <device-id>
dotnet run --project src\Fling.Cli -- stop <device-id>
```

Pairing credentials are managed by pyatv's per-user storage. `cast-file` stays
running because Apple TV fetches the file from the temporary LAN server.

## Walk: desktop streaming

```powershell
dotnet run --project src\Fling.Cli -- mirror <device-id>
```

Press Ctrl+C to stop FFmpeg and the media server. The optional desktop number
is passed to FFmpeg's `gdigrab` source, but multi-monitor behavior varies by
FFmpeg build; full virtual-desktop capture is the reliable starting point.

Current tuning favors latency over compression efficiency: 30 fps H.264,
one-second keyframes, and a six-segment rolling HLS playlist. Apple TV may add
its own buffer, so this is casting with several seconds of latency rather than
interactive mirroring.

## Validation

```powershell
dotnet test
py -m unittest discover -s bridge\tests -v
```

See [architecture](docs/architecture.md) and
[implementation notes](docs/implementation-notes.md) for boundaries,
decisions, limitations, and the path toward a native implementation.
