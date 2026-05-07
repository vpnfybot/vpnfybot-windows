# WireGuard Split-Tunneling Solution 👻
### Powered by WireProxy + ProxyBridge + WinDivert

<p align="center">
  <img src="https://github.com/vpnfybot/vpnfybot-windows/blob/main/src/interface.png?raw=true" width="320" height="410">
</p>

<p align="center">
  <a href="README.ru.md">RU</a>
</p>

<p align="center">
  <a href="LICENSE">
    <img src="https://img.shields.io/badge/License-MIT-yellow.svg">
  </a>
</p>

---

## 🚀 About

A lightweight Windows application that brings **true split tunneling** to WireGuard
Route only selected apps and websites through your VPN — or exclude them completely

Built on top of:
- **WireProxy**
- **ProxyBridge**
- **WinDivert**

---

## ✨ Features

- 🎯 **App-based routing**  
  Select which applications should use the VPN tunnel

- 🌐 **Website-based routing**  
  Route specific domains through VPN

- 🚫 **Exclusion rules**  
  Exclude apps or websites from VPN traffic

- 🔄 **Automatic updates**  
  Built-in version checker with seamless updates

- 🌍 **Multi-language support**  
  Available in:
  - 🇺🇸 English  
  - 🇷🇺 Russian  

- ⚡ **Simple configuration**  
  Just import your `.conf` file and connect (.conf file can be obtained for free from <a href="https://t.me/vpnfybot">@vpnfybot</a>)

---

## 📦 Installation

1. Download the latest release from GitHub  
2. Run the installer `.exe` file  
3. Import your configuration  
4. Done ✅

---

## 🛠 Build installer

Run `src\build-installer-release.bat` to produce `src\dist\vpnfybot-windows.exe`.

The installer:
- installs the app into the current user's local Programs directory
- keeps logs, cache, configs, updater files, and extracted dependencies inside the installed app folder
- preinstalls `wireproxy.exe` and `ProxyBridge_CLI.exe` into a stable `deps` path and adds Windows Firewall rules for them

---

## ⚠️ Notes

- Some applications or websites may behave unexpectedly due to OS/network limitations  
- Administrator privileges may be required  
- Uses packet-level filtering via WinDivert

---

## 🧠 Why This Exists

WireGuard does not provide native split tunneling for apps/domains on Windows.  
This tool fills that gap with a flexible and user-friendly solution.

---

<p align="center">
  Made with ❤️ from <a href="https://t.me/vpnfybot">@vpnfybot</a>
</p>
