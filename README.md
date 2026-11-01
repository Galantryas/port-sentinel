# Port Sentinel

A small desktop utility that answers one question: *what is running on this machine and which port is it holding?*

It lives in the system tray with no taskbar entry. Open it and you get live system stats, every listening TCP/UDP port with the process that owns it, and the processes using the most memory. Dev ports — 3000, 5173, 8080 and friends — are flagged so you can spot a stray dev server at a glance.

## What it does

- **Ports tab** — listening ports, protocol, owning PID, process name, executable path. Filterable by text, refreshed every 1 or 3 seconds, or paused.
- **Processes tab** — CPU and memory totals, plus the heaviest processes by resident memory.
- **Kill** — terminate the process holding a port without opening Task Manager.
- **Tray behaviour** — hides to the tray, opens on click, closes without leaving an orphan.

## Stack

| Layer | Choice |
| --- | --- |
| Shell | Tauri 2 |
| Frontend | Svelte 5 (runes), TypeScript, Vite, Tailwind |
| Backend | Rust, `sysinfo` |
| Platform | Windows (primary); the Rust side is portable where `sysinfo` supports it |

## Running

Prerequisites: Node 20+, Rust toolchain, and the Tauri prerequisites for your platform.

```bash
npm install
npm run tauri dev
```

Build a release binary:

```bash
npm run tauri build
```

## Project layout

```
src/                  Svelte frontend
  App.svelte          Tabs, polling, table rendering
src-tauri/
  src/lib.rs          Tauri commands: stats, ports, kill
  src/main.rs         Entry point
  tauri.conf.json     Window, tray, and bundle configuration
```

## Notes

Killing a process is immediate and unsaved work is lost — the app asks for confirmation before it does it. Port and process enumeration is read from the OS, so some rows require running the app with elevated privileges on Windows.

## Licence

Personal project. Not licensed for redistribution.
