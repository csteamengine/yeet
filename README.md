# Yoink

A lightweight clipboard manager for macOS built with Tauri 2, React, and Rust.

## Features

- **Clipboard history** — automatically captures text, URLs, code snippets, files, and images
- **Image support** — copies images from browsers/websites are saved and previewed
- **Preview panel** — split-pane UI with list on the left and full content preview on the right
- **Hotkey mode** — Cmd+Shift+V opens the panel; cycle with V, release modifiers to paste
- **Smart detection** — auto-classifies content as text, URL, code, file, or image
- **Excluded apps** — skip captures from specific apps (e.g. password managers)
- **Paste interception** — optionally intercept Cmd+V to paste from history
- **NSPasteboard privacy** — respects transient, auto-generated, and concealed markers

## Architecture

- **Backend**: Rust poll thread reads NSPasteboard every 150ms via native Objective-C calls, deduplicates by content hash, stores in SQLite
- **Frontend**: React + Zustand, refreshes from DB every 300ms
- **Images**: saved as PNG files in `~/Library/Application Support/com.yoink.app/images/`, served to frontend as base64 data URLs

## Development

```bash
pnpm install
pnpm tauri dev
```

## Build

```bash
pnpm tauri build
```

## Stack

- [Tauri 2](https://tauri.app) — app framework
- [React](https://react.dev) + [Vite](https://vitejs.dev) — frontend
- [Zustand](https://github.com/pmndrs/zustand) — state management
- [Tailwind CSS](https://tailwindcss.com) — styling
- [SQLite](https://www.sqlite.org/) (via rusqlite) — persistence
