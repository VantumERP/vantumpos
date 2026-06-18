# VantumPOS

Local-first POS foundation for Serbian retail stores and boutiques. The current build uses React, shadcn/ui, Tauri, Rust, and SQLite with no fiscalization or cloud integrations enabled yet.

## Development

- `bun run dev` starts the Vite frontend.
- `bun run tauri dev` starts the desktop app.
- `bun run test` runs frontend tests.
- `bun run build` builds the frontend.
- `cd src-tauri && cargo test -- --test-threads=1` runs Rust tests.
