# WireGuard split-tunneling solution powered by WireProxy + ProxyBridge w/ WinDivert

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE) [![CI](https://img.shields.io/github/actions/workflow/status/USERNAME/REPO/ci.yml?branch=main)](https://github.com/USERNAME/REPO/actions) [![Latest Release](https://img.shields.io/github/v/release/USERNAME/REPO?label=release)](https://github.com/USERNAME/REPO/releases)

A Windows-focused WireGuard split-tunneling toolkit combining WireProxy, ProxyBridge and WinDivert to provide per-process and per-site routing through a WireGuard tunnel.

- WireProxy: https://github.com/windtf/wireproxy
- ProxyBridge: https://github.com/InterceptSuite/ProxyBridge
- WinDivert: https://github.com/basil00/WinDivert

Overview

This project implements a practical Windows solution for WireGuard split-tunneling, enabling you to route only selected applications or sites through a WireGuard tunnel while leaving other traffic on the normal network interface.

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
