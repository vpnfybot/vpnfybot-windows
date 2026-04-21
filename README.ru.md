# WireGuard split-tunneling solution powered by WireProxy + ProxyBridge w/ WinDivert 

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE) [![CI](https://img.shields.io/github/actions/workflow/status/USERNAME/REPO/ci.yml?branch=main)](https://github.com/USERNAME/REPO/actions) [![Latest Release](https://img.shields.io/github/v/release/USERNAME/REPO?label=release)](https://github.com/USERNAME/REPO/releases)

Windows-решение для раздельного туннелирования (split-tunneling) через WireGuard, использующее WireProxy, ProxyBridge и WinDivert для маршрутизации трафика по процессам и сайтам.

Быстрые ссылки
- WireProxy: https://github.com/windtf/wireproxy
- ProxyBridge: https://github.com/InterceptSuite/ProxyBridge
- WinDivert: https://github.com/basil00/WinDivert

Обзор

Проект предоставляет удобный GUI и вспомогательные бинарники для настройки раздельного туннелирования на Windows. Вы можете направлять трафик конкретных приложений или сайтов через WireGuard, оставляя остальной трафик вне туннеля.

Функции

- Раздельное туннелирование по процессам и по сайтам.
- Подсчёт трафика только внутри туннеля на основе `/metrics` WireProxy (пер-пир `tx_bytes` / `rx_bytes`).
- Расчёт скоростей и сессий (обновления 1 раз в секунду).
- Автоматическое переключение MB→GB при больших объёмах.
- Нативный рендер текста на Windows (GDI/HFONT → bitmap → GPU) для чёткой отрисовки.
- DPI-aware верстка, привязка иконок и привязка к пиксельной сетке для устранения размытия.

Коротко о работе

1. WireProxy поддерживает подключение peer'а WireGuard и предоставляет `/metrics`.
2. GUI опрашивает `/metrics` каждую секунду, вычисляет дельты и показывает скорости и суммарный трафик.
3. ProxyBridge и (при необходимости) WinDivert перенаправляют выбранный трафик в туннель.

Быстрый старт (Windows)

```powershell
cd src
cargo build --release --bin vpnfybot-windows
.\target\release\vpnfybot-windows.exe
```

Лицензии третьих сторон

См. `wireproxy-master/LICENSE`, `proxybridge-master/LICENSE`, `WinDivert-2.2.2-A/LICENSE`.

Лицензия проекта: MIT (см. `LICENSE`).
