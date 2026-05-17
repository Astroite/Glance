# Glance

A local-first photo timeline for photographers.

## Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [pnpm](https://pnpm.io/)
- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- Windows: WebView2 (included with Windows 10/11)

## Development

```bash
# Install dependencies
pnpm install

# Start development server (frontend + Tauri)
pnpm tauri dev

# Build frontend only
pnpm build

# Run Rust tests
cd src-tauri && cargo test

# Lint frontend
pnpm lint
```

## Project Structure

```
Glance/
├── src/                    # React + TypeScript frontend
│   ├── views/              # Page-level components
│   ├── components/         # Reusable UI components
│   ├── ipc/                # Tauri command wrappers
│   ├── state/              # Global state management
│   └── styles/             # CSS styles
├── src-tauri/              # Tauri + Rust backend
│   ├── src/
│   │   ├── main.rs         # Application entry point
│   │   ├── lib.rs          # Tauri setup and command registration
│   │   ├── commands/       # Tauri command handlers
│   │   ├── core/           # Core business logic
│   │   │   ├── scanner/    # Directory traversal
│   │   │   ├── identity/   # File identity (hash + size)
│   │   │   ├── exif/       # EXIF + XMP extraction
│   │   │   ├── thumbnail/  # Thumbnail generation
│   │   │   ├── raw/        # RAW file handling
│   │   │   ├── db/         # SQLite database
│   │   │   └── tasks/      # Background task queue
│   │   └── error.rs        # Error types
│   └── tauri.conf.json     # Tauri configuration
└── Docs/                   # Design documents
```

## Tech Stack

- **Desktop**: Tauri v2 (WebView2)
- **Backend**: Rust
- **Frontend**: React + TypeScript + Vite
- **Database**: SQLite (WAL mode)
- **Virtual scrolling**: react-virtuoso
