# wireproxy client binaries

`wireproxy.exe` is the Windows amd64 build from:

https://github.com/artem-russkikh/wireproxy-awg/releases/tag/v1.0.15

Release archive used: `wireproxy_windows_amd64.tar.gz`

Published archive SHA256:

`ab272758826fbe5ca8a86b7fade7131e9df1e1e9939197b2d108908ce66e33fe`

Extracted `wireproxy.exe` SHA256:

`643f3a9bce7e4e0f3e347bff44227e267d5f4b9236e168bfb233aafabae80799`

`wireproxy.exe` remains the compatibility client for standard WireGuard and
AmneziaWG servers.

## HyperWG client

`wireproxy-hyperwg.exe` is a Windows amd64 build of the same wireproxy-awg
source commit `380913b8eac28d609379d4bb14f4ab8d2fd6c3ad`, linked to the HyperWG
overlay on AmneziaWG Go 3.1 commit
`b5928efb6ca19f0153958460c3d141f04abc5c2e` (`v3.1.20260828`). The overlay
reports `3.1-hyperwg-morph2-perf5` and implements mandatory HyperWG v2
handshake trailers/pacing plus `TrafficMorpher`.

The wireproxy config adapter additionally serializes `TrafficMorpher`,
`HeaderProtectionKey`, `ContentPaddingAddition`, timing ranges,
`RandomTrailers`, and `DisableCookies` to the AWG 3.1 UAPI. Header-protection
keys are accepted in the native base64 form and converted to the UAPI hex form.

Build toolchain: Go 1.26.5 Windows amd64. The official archive SHA256 was
`97e6b2a833b6d89f9ff17d25419ac0a7e3b482a044e9ab18cdef834bd834fd38`.

`wireproxy-hyperwg.exe` SHA256:

`a79cd4141703b44679577ef411598e0edc970219797f1211959fabdf66f68277`

The application selects this binary only when the runtime `[Interface]`
contains `TrafficMorpher`; ordinary WG/AWG connections keep using the original
binary.
