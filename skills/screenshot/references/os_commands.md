# Screenshot OS-Specific Commands

## Rust helper

```bash
cargo run --manifest-path <repo-root>/rust_tools/screenshot_rs/Cargo.toml --release --  # default location
cargo run --manifest-path <repo-root>/rust_tools/screenshot_rs/Cargo.toml --release -- --mode temp
cargo run --manifest-path <repo-root>/rust_tools/screenshot_rs/Cargo.toml --release -- --path output/screen.png
cargo run --manifest-path <repo-root>/rust_tools/screenshot_rs/Cargo.toml --release -- --app "Codex"
cargo run --manifest-path <repo-root>/rust_tools/screenshot_rs/Cargo.toml --release -- --app "Codex" --window-name "Settings"
cargo run --manifest-path <repo-root>/rust_tools/screenshot_rs/Cargo.toml --release -- --list-windows --app "Codex"
cargo run --manifest-path <repo-root>/rust_tools/screenshot_rs/Cargo.toml --release -- --mode temp --region 100,200,800,600
cargo run --manifest-path <repo-root>/rust_tools/screenshot_rs/Cargo.toml --release -- --mode temp --active-window
cargo run --manifest-path <repo-root>/rust_tools/screenshot_rs/Cargo.toml --release -- --window-id 12345
```

Combine preflight + capture:
```bash
cargo run --manifest-path <repo-root>/rust_tools/screenshot_rs/Cargo.toml --release -- \
  --ensure-macos-permissions && \
cargo run --manifest-path <repo-root>/rust_tools/screenshot_rs/Cargo.toml --release -- \
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
