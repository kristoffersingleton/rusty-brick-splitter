# PDF Combiner

A fast, lightweight desktop application for combining multiple PDF files into one or splitting them apart. Built with Tauri (Rust) and Svelte.

## Features

- Select multiple PDF files via native file picker
- Drag-and-drop reordering of files
- View page count for each PDF
- Combine into a single PDF with one click
- Dark theme UI
- Small binary size (~10-20MB)

## Quick Start

### Prerequisites

- Node.js (v18+)
- Rust (via rustup)
- Tauri CLI: `cargo install tauri-cli`

**Linux/WSL additional dependencies:**
```bash
sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

### Development

```bash
# Install dependencies
npm install

# Run in development mode (hot reload)
npm run tauri dev
```

First run will compile the Rust backend which takes a few minutes. Subsequent runs are fast.

### Build for Production

```bash
npm run tauri build
```

Output binary location: `src-tauri/target/release/bundle/`

- **Linux**: `.deb` and `.AppImage` in `bundle/deb/` and `bundle/appimage/`
- **Windows**: `.msi` and `.exe` in `bundle/msi/` and `bundle/nsis/`
- **macOS**: `.dmg` and `.app` in `bundle/dmg/` and `bundle/macos/`

## Project Structure

```
pdf-combiner/
├── src/                      # Frontend (Svelte)
│   ├── routes/
│   │   └── +page.svelte      # Main UI component
│   └── app.css               # Tailwind CSS
├── src-tauri/                # Backend (Rust)
│   ├── src/
│   │   └── lib.rs            # PDF combining logic
│   ├── Cargo.toml            # Rust dependencies
│   └── tauri.conf.json       # Tauri configuration
├── package.json
└── vite.config.js
```

## Tech Stack

| Layer | Technology |
|-------|------------|
| Frontend | Svelte 5, TypeScript, Tailwind CSS |
| Backend | Rust, lopdf |
| Framework | Tauri 2 |
| Build | Vite, Cargo |

## How It Works

1. **File Selection**: Uses Tauri's native dialog plugin for OS file pickers
2. **PDF Parsing**: Rust backend uses `lopdf` to read PDF structure and page counts
3. **Combining**: Objects from each PDF are remapped to new IDs and merged into a single document
4. **Output**: Combined PDF is compressed and saved to user-selected location

## Commands

| Command | Description |
|---------|-------------|
| `npm run tauri dev` | Start development server with hot reload |
| `npm run tauri build` | Build production binary |
| `npm run build` | Build frontend only |
| `npm run dev` | Run frontend dev server only (no Tauri) |

## Configuration

### Window Settings

Edit `src-tauri/tauri.conf.json`:
```json
{
  "app": {
    "windows": [{
      "title": "PDF Combiner",
      "width": 700,
      "height": 600,
      "minWidth": 500,
      "minHeight": 400
    }]
  }
}
```

### App Metadata

Edit `src-tauri/Cargo.toml` for Rust package info and `src-tauri/tauri.conf.json` for app identity.

## WSL Notes

If running on WSL, you need a display server:
- **Windows 11**: WSLg is built-in, should work automatically
- **Windows 10**: Install VcXsrv or similar X server

## Recommended IDE Setup

[VS Code](https://code.visualstudio.com/) with extensions:
- [Svelte](https://marketplace.visualstudio.com/items?itemName=svelte.svelte-vscode)
- [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode)
- [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
- [Tailwind CSS IntelliSense](https://marketplace.visualstudio.com/items?itemName=bradlc.vscode-tailwindcss)

## License

MIT
