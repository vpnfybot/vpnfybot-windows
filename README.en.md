# WireGuard split-tunneling solution powered by WireProxy + ProxyBridge w/ WinDivert

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE) [![CI](https://img.shields.io/github/actions/workflow/status/USERNAME/REPO/ci.yml?branch=main)](https://github.com/USERNAME/REPO/actions) [![Latest Release](https://img.shields.io/github/v/release/USERNAME/REPO?label=release)](https://github.com/USERNAME/REPO/releases)

A Windows-focused WireGuard split-tunneling toolkit combining WireProxy, ProxyBridge and WinDivert to provide per-process and per-site routing through a WireGuard tunnel.

Quick links
- WireProxy: https://github.com/windtf/wireproxy
- ProxyBridge: https://github.com/InterceptSuite/ProxyBridge
- WinDivert: https://github.com/basil00/WinDivert

Overview

This project implements a practical Windows solution for WireGuard split-tunneling, enabling you to route only selected applications or sites through a WireGuard tunnel while leaving other traffic on the normal network interface.

Features

- Per-process and per-site split-tunneling controls.
- Tunnel-only traffic accounting using WireProxy `/metrics` endpoint (per-peer `tx_bytes` / `rx_bytes`).
- Speeds and session totals computed from tunneled counters, updated once per second.
- MB→GB unit switch when values exceed 1000 MB.
- Native-looking text rendering on Windows using GDI/HFONT → bitmap → GPU texture pipeline for pixel-crisp UI.
- DPI-aware layout, icon pinning and pixel-snapped rendering to avoid blurred text.

How it works (brief)

1. WireProxy manages the WireGuard peer connection and exposes a `/metrics` endpoint for totals.
2. The GUI polls `/metrics` once per second and computes session deltas (upload/download bps and totals).
3. ProxyBridge (together with WinDivert when required) forwards selected application traffic into the WireGuard tunnel.

Quick start (Windows)

1. Build the GUI and binary from the `src` folder:

```powershell
cd src
cargo build --release --bin vpnfybot-windows
```

2. Run the built executable:

```powershell
.\target\release\vpnfybot-windows.exe
```

Third-party components & licenses

This repository includes or references third-party projects. Their original licenses are preserved in their respective folders:

- WireProxy — `wireproxy-master/LICENSE`
- ProxyBridge — `proxybridge-master/LICENSE`
- WinDivert — `WinDivert-2.2.2-A/LICENSE`

License (this project)

This project's code is licensed under the MIT License — see `LICENSE`.

Contributing

Contributions are welcome. Before opening pull requests, please confirm that any changes to bundled third-party components comply with their licenses.

SEO keywords

wireguard split-tunneling, wireguard split tunneling, wireguard раздельное туннелирование, split tunneling, Windows WireGuard GUI, WireProxy, ProxyBridge, WinDivert
