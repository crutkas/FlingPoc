# Third-party notices

## pyatv 0.18.0

AirPlay discovery, credential, RTSP, binary-plist, URL-playback, and timing
behavior was studied from pyatv 0.18.0. The Rust implementation is a new
implementation, but this notice is retained for the permissively licensed
protocol reference and future ports.

> The MIT License (MIT)
>
> Copyright (c) 2020 Pierre Ståhl
>
> Permission is hereby granted, free of charge, to any person obtaining a copy
> of this software and associated documentation files (the "Software"), to deal
> in the Software without restriction, including without limitation the rights
> to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
> copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in
> all copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
> SOFTWARE.

Source: <https://github.com/postlund/pyatv>, tag `v0.18.0`.

## Direct Rust dependencies

The workspace directly uses these permissively licensed crates:

| Crate | Version | License |
| --- | --- | --- |
| axum | 0.8.9 | MIT |
| constant_time_eq | 0.5.0 | CC0-1.0 OR MIT-0 OR Apache-2.0 |
| mdns-sd | 0.21.0 | MIT OR Apache-2.0 |
| plist | 1.10.0 | MIT |
| serde / serde_json | 1.0.229 / 1.0.151 | MIT OR Apache-2.0 |
| thiserror | 2.0.20 | MIT OR Apache-2.0 |
| tokio | 1.53.1 | MIT |
| tower | 0.5.3 | MIT |

The exact transitive dependency graph and checksums are recorded in
`Cargo.lock`. No GPL/LGPL AirPlay implementation or FairPlay implementation is
included.

