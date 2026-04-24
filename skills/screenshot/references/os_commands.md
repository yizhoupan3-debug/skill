# Screenshot OS-Specific Commands

## Rust helper

```bash
cargo run --manifest-path <repo-root>/rust_tools/Cargo.toml --release --bin screenshot --  # default location
cargo run --manifest-path <repo-root>/rust_tools/Cargo.toml --release --bin screenshot -- --mode temp
cargo run --manifest-path <repo-root>/rust_tools/Cargo.toml --release --bin screenshot -- --path output/screen.png
cargo run --manifest-path <repo-root>/rust_tools/Cargo.toml --release --bin screenshot -- --app "Codex"
cargo run --manifest-path <repo-root>/rust_tools/Cargo.toml --release --bin screenshot -- --app "Codex" --window-name "Settings"
cargo run --manifest-path <repo-root>/rust_tools/Cargo.toml --release --bin screenshot -- --list-windows --app "Codex"
cargo run --manifest-path <repo-root>/rust_tools/Cargo.toml --release --bin screenshot -- --mode temp --region 100,200,800,600
cargo run --manifest-path <repo-root>/rust_tools/Cargo.toml --release --bin screenshot -- --mode temp --active-window
cargo run --manifest-path <repo-root>/rust_tools/Cargo.toml --release --bin screenshot -- --window-id 12345
cargo run --manifest-path <repo-root>/rust_tools/Cargo.toml --release --bin screenshot -- --mode temp --json
```

Combine preflight + capture:
```bash
cargo run --manifest-path <repo-root>/rust_tools/Cargo.toml --release --bin screenshot -- \
  --ensure-macos-permissions && \
cargo run --manifest-path <repo-root>/rust_tools/Cargo.toml --release --bin screenshot -- \
  --app "<App>" --mode temp
```

### Multi-display: full-screen saves one file per display on macOS and one stitched image on other platforms.

## macOS direct commands (fallback)

```bash
screencapture -x output/screen.png                 # full screen
screencapture -x -R100,200,800,600 output/region.png  # region
screencapture -x -l12345 output/window.png          # window id
screencapture -x -i output/interactive.png          # interactive selection
```

## Linux

Prerequisites: `scrot`, `gnome-screenshot`, or ImageMagick `import`.

```bash
scrot output/screen.png                  # full screen
scrot -a 100,200,800,600 output/region.png  # region
scrot -u output/window.png               # active window

gnome-screenshot -f output/screen.png    # full screen
gnome-screenshot -w -f output/window.png # active window

import -window root output/screen.png    # full screen (ImageMagick)
import -window root -crop 800x600+100+200 output/region.png  # region
```

`--app`, `--window-name`, `--list-windows`, and `--interactive` are macOS-only. On Linux and Windows use `--active-window`, `--window-id`, full-screen, or `--region`.
