# Screenshot OS-Specific Commands

## macOS (Python helper)

```bash
python3 <path-to-skill>/scripts/take_screenshot.py                           # default location
python3 <path-to-skill>/scripts/take_screenshot.py --mode temp               # temp (Codex inspection)
python3 <path-to-skill>/scripts/take_screenshot.py --path output/screen.png  # explicit path
python3 <path-to-skill>/scripts/take_screenshot.py --app "Codex"             # app window capture
python3 <path-to-skill>/scripts/take_screenshot.py --app "Codex" --window-name "Settings"  # specific window
python3 <path-to-skill>/scripts/take_screenshot.py --list-windows --app "Codex"   # list windows
python3 <path-to-skill>/scripts/take_screenshot.py --mode temp --region 100,200,800,600  # region
python3 <path-to-skill>/scripts/take_screenshot.py --mode temp --active-window   # focused window
python3 <path-to-skill>/scripts/take_screenshot.py --window-id 12345              # specific window id
```

Combine preflight + capture:
```bash
bash <path-to-skill>/scripts/ensure_macos_permissions.sh && \
python3 <path-to-skill>/scripts/take_screenshot.py --app "<App>" --mode temp
```

### Multi-display: full-screen saves one file per display on macOS.

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

`--app`, `--window-name`, `--list-windows` are macOS-only. On Linux use `--active-window` or `--window-id`.

## Windows (PowerShell helper)

```powershell
powershell -ExecutionPolicy Bypass -File <path-to-skill>/scripts/take_screenshot.ps1              # default
powershell -ExecutionPolicy Bypass -File <path-to-skill>/scripts/take_screenshot.ps1 -Mode temp   # temp
powershell -ExecutionPolicy Bypass -File <path-to-skill>/scripts/take_screenshot.ps1 -Path "C:\Temp\screen.png"  # explicit
powershell -ExecutionPolicy Bypass -File <path-to-skill>/scripts/take_screenshot.ps1 -Mode temp -Region 100,200,800,600  # region
powershell -ExecutionPolicy Bypass -File <path-to-skill>/scripts/take_screenshot.ps1 -Mode temp -ActiveWindow  # active window
powershell -ExecutionPolicy Bypass -File <path-to-skill>/scripts/take_screenshot.ps1 -WindowHandle 123456  # specific handle
```
