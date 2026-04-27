use anyhow::{anyhow, bail, Context, Result};
use chrono::Local;
use clap::Parser;
use image::{imageops, DynamicImage, ImageBuffer, ImageFormat, Rgba, RgbaImage};
use serde_json::json;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use xcap::{Monitor, Window};

const TEST_MODE_ENV: &str = "CODEX_SCREENSHOT_TEST_MODE";
const TEST_PLATFORM_ENV: &str = "CODEX_SCREENSHOT_TEST_PLATFORM";
const TEST_WINDOWS_ENV: &str = "CODEX_SCREENSHOT_TEST_WINDOWS";
const TEST_DISPLAYS_ENV: &str = "CODEX_SCREENSHOT_TEST_DISPLAYS";
const MACOS: &str = "macos";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Region {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone)]
struct WindowMeta {
    id: u32,
    owner: String,
    title: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    layer: i32,
    focused: bool,
}

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Cross-platform screenshot helper for Codex skills"
)]
struct Cli {
    #[arg(long)]
    path: Option<String>,

    #[arg(long, default_value = "default", value_parser = ["default", "temp"])]
    mode: String,

    #[arg(long, default_value = "png")]
    format: String,

    #[arg(long)]
    app: Option<String>,

    #[arg(long)]
    window_name: Option<String>,

    #[arg(long)]
    list_windows: bool,

    #[arg(long, value_parser = parse_region)]
    region: Option<Region>,

    #[arg(long)]
    window_id: Option<u32>,

    #[arg(long)]
    active_window: bool,

    #[arg(long)]
    interactive: bool,

    #[arg(long)]
    ensure_macos_permissions: bool,

    #[arg(long)]
    json: bool,
}

fn main() {
    if let Err(err) = run_cli() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run_cli() -> Result<()> {
    let args = Cli::parse();
    validate_args(&args)?;

    if args.ensure_macos_permissions {
        ensure_macos_permissions()?;
        return Ok(());
    }

    let system = test_platform_override().unwrap_or_else(|| env::consts::OS.to_string());

    validate_platform_args(&args, &system)?;

    if test_mode_enabled() {
        return run_test_mode(&args, &system);
    }

    if is_macos(&system) {
        ensure_macos_permissions()?;
    }

    if args.list_windows {
        let windows = list_windows(&args)?;
        emit_window_list(&windows, args.json)?;
        return Ok(());
    }

    let output = resolve_output_path(args.path.as_deref(), &args.mode, &args.format, &system)?;

    if args.interactive {
        capture_interactive(&args, &output, &system)?;
        emit_capture_paths(&[output], &args, &system)?;
        return Ok(());
    }

    let paths = capture(&args, &output, &system)?;
    emit_capture_paths(&paths, &args, &system)?;
    Ok(())
}

fn validate_platform_args(args: &Cli, system: &str) -> Result<()> {
    if !is_macos(system) && (args.app.is_some() || args.window_name.is_some() || args.list_windows)
    {
        bail!("--app/--window-name/--list-windows are supported on macOS only");
    }
    Ok(())
}

fn validate_args(args: &Cli) -> Result<()> {
    if args.region.is_some() && args.window_id.is_some() {
        bail!("choose either --region or --window-id, not both");
    }
    if args.region.is_some() && args.active_window {
        bail!("choose either --region or --active-window, not both");
    }
    if args.window_id.is_some() && args.active_window {
        bail!("choose either --window-id or --active-window, not both");
    }
    if args.app.is_some() && args.window_id.is_some() {
        bail!("choose either --app or --window-id, not both");
    }
    if args.region.is_some() && args.app.is_some() {
        bail!("choose either --region or --app, not both");
    }
    if args.region.is_some() && args.window_name.is_some() {
        bail!("choose either --region or --window-name, not both");
    }
    if args.interactive && args.app.is_some() {
        bail!("choose either --interactive or --app, not both");
    }
    if args.interactive && args.window_name.is_some() {
        bail!("choose either --interactive or --window-name, not both");
    }
    if args.interactive && args.window_id.is_some() {
        bail!("choose either --interactive or --window-id, not both");
    }
    if args.interactive && args.active_window {
        bail!("choose either --interactive or --active-window, not both");
    }
    if args.list_windows && (args.region.is_some() || args.window_id.is_some() || args.interactive)
    {
        bail!("--list-windows only supports --app, --window-name, and --active-window");
    }
    Ok(())
}

fn parse_region(value: &str) -> Result<Region, String> {
    let parts: Vec<&str> = value.split(',').map(str::trim).collect();
    if parts.len() != 4 {
        return Err("region must be x,y,w,h".to_string());
    }
    let x = parts[0]
        .parse::<i32>()
        .map_err(|_| "region values must be integers".to_string())?;
    let y = parts[1]
        .parse::<i32>()
        .map_err(|_| "region values must be integers".to_string())?;
    let width = parts[2]
        .parse::<u32>()
        .map_err(|_| "region values must be integers".to_string())?;
    let height = parts[3]
        .parse::<u32>()
        .map_err(|_| "region values must be integers".to_string())?;
    if width == 0 || height == 0 {
        return Err("region width and height must be positive".to_string());
    }
    Ok(Region {
        x,
        y,
        width,
        height,
    })
}

fn test_mode_enabled() -> bool {
    env::var(TEST_MODE_ENV)
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

fn test_platform_override() -> Option<String> {
    env::var(TEST_PLATFORM_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| normalize_platform(&value))
}

fn normalize_platform(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "darwin" | "mac" | "macos" | "osx" => MACOS.to_string(),
        "windows" | "win" => "windows".to_string(),
        "linux" | "ubuntu" => "linux".to_string(),
        other => other.to_string(),
    }
}

fn parse_int_list(value: &str) -> Vec<u32> {
    value
        .split(',')
        .filter_map(|part| part.trim().parse::<u32>().ok())
        .collect()
}

fn test_window_ids() -> Vec<u32> {
    let ids = env::var(TEST_WINDOWS_ENV)
        .ok()
        .map(|value| parse_int_list(&value))
        .unwrap_or_else(|| vec![101, 102]);
    if ids.is_empty() {
        vec![101]
    } else {
        ids
    }
}

fn test_display_ids() -> Vec<u32> {
    let ids = env::var(TEST_DISPLAYS_ENV)
        .ok()
        .map(|value| parse_int_list(&value))
        .unwrap_or_else(|| vec![1, 2]);
    if ids.is_empty() {
        vec![1]
    } else {
        ids
    }
}

fn run_test_mode(args: &Cli, system: &str) -> Result<()> {
    if args.list_windows {
        let windows = list_test_windows(args);
        emit_window_list(&windows, args.json)?;
        return Ok(());
    }

    let output = resolve_output_path(args.path.as_deref(), &args.mode, &args.format, system)?;
    let mut paths = vec![output.clone()];

    if args.window_id.is_some()
        || (is_macos(system)
            && (args.app.is_some() || args.window_name.is_some() || args.active_window))
    {
        let ids = test_window_targets(args);
        paths = multi_output_paths(
            &output,
            &ids.iter().map(|id| format!("w{id}")).collect::<Vec<_>>(),
        )?;
    } else if is_macos(system) && args.region.is_none() && !args.interactive {
        let ids = test_display_ids();
        if ids.len() > 1 {
            paths = multi_output_paths(
                &output,
                &ids.iter().map(|id| format!("d{id}")).collect::<Vec<_>>(),
            )?;
        }
    }

    for path in &paths {
        write_test_image(path, &args.format)?;
    }
    emit_capture_paths(&paths, args, system)?;
    Ok(())
}

fn list_test_windows(args: &Cli) -> Vec<WindowMeta> {
    let owner = args.app.as_deref().unwrap_or("TestApp");
    let title = args.window_name.as_deref().unwrap_or("");
    let ids: Vec<u32> = if args.active_window {
        test_window_ids().into_iter().take(1).collect()
    } else {
        test_window_ids()
    };
    ids.into_iter()
        .enumerate()
        .map(|(idx, id)| WindowMeta {
            id,
            owner: owner.to_string(),
            title: if title.is_empty() {
                format!("Window {}", idx + 1)
            } else {
                title.to_string()
            },
            x: 0,
            y: 0,
            width: 800,
            height: 600,
            layer: 0,
            focused: idx == 0,
        })
        .collect()
}

fn test_window_targets(args: &Cli) -> Vec<u32> {
    if let Some(id) = args.window_id {
        vec![id]
    } else if args.active_window {
        test_window_ids().into_iter().take(1).collect()
    } else {
        test_window_ids()
    }
}

fn timestamp() -> String {
    Local::now().format("%Y-%m-%d_%H-%M-%S").to_string()
}

fn default_filename(fmt: &str, prefix: &str) -> String {
    format!("{prefix}-{}.{}", timestamp(), normalized_format(fmt))
}

fn normalized_format(fmt: &str) -> String {
    fmt.trim().trim_start_matches('.').to_ascii_lowercase()
}

fn resolve_output_path(
    requested_path: Option<&str>,
    mode: &str,
    fmt: &str,
    system: &str,
) -> Result<PathBuf> {
    let fmt = normalized_format(fmt);
    if let Some(requested) = requested_path {
        let mut path = expand_path(requested);
        if path.exists() && path.is_dir() {
            path = path.join(default_filename(&fmt, "screenshot"));
        } else if requested.ends_with('/') || requested.ends_with('\\') {
            std::fs::create_dir_all(&path).with_context(|| {
                format!("failed to create output directory: {}", path.display())
            })?;
            path = path.join(default_filename(&fmt, "screenshot"));
        } else if path.extension().is_none() {
            path.set_extension(&fmt);
        }
        ensure_parent(&path)?;
        return Ok(path);
    }

    let path = if mode == "temp" {
        env::temp_dir().join(default_filename(&fmt, "codex-shot"))
    } else {
        default_dir(system).join(default_filename(&fmt, "screenshot"))
    };
    ensure_parent(&path)?;
    Ok(path)
}

fn expand_path(value: &str) -> PathBuf {
    if value == "~" {
        return home_dir();
    }
    if let Some(rest) = value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix("~\\"))
    {
        return home_dir().join(rest);
    }
    PathBuf::from(value)
}

fn home_dir() -> PathBuf {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn default_dir(system: &str) -> PathBuf {
    let home = home_dir();
    if is_macos(system) {
        if let Ok(output) = Command::new("defaults")
            .args(["read", "com.apple.screencapture", "location"])
            .output()
        {
            if output.status.success() {
                let location = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !location.is_empty() {
                    return expand_path(&location);
                }
            }
        }
        return home.join("Desktop");
    }

    let pictures = home.join("Pictures");
    let screenshots = pictures.join("Screenshots");
    if screenshots.exists() {
        screenshots
    } else if pictures.exists() {
        pictures
    } else {
        home
    }
}

fn ensure_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("failed to create output directory: {}", parent.display())
            })?;
        }
    }
    Ok(())
}

fn multi_output_paths(base: &Path, suffixes: &[String]) -> Result<Vec<PathBuf>> {
    if suffixes.len() <= 1 {
        return Ok(vec![base.to_path_buf()]);
    }

    let stem = base
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow!("invalid output path: {}", base.display()))?;
    let ext = base
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let mut paths = Vec::with_capacity(suffixes.len());
    for suffix in suffixes {
        let filename = if ext.is_empty() {
            format!("{stem}-{suffix}")
        } else {
            format!("{stem}-{suffix}.{ext}")
        };
        let path = base.with_file_name(filename);
        ensure_parent(&path)?;
        paths.push(path);
    }
    Ok(paths)
}

fn capture(args: &Cli, output: &Path, system: &str) -> Result<Vec<PathBuf>> {
    if let Some(window_id) = args.window_id {
        let window = find_window_by_id(window_id)?;
        let image = window.capture_image().context("failed to capture window")?;
        save_image(&image, output, &args.format)?;
        return Ok(vec![output.to_path_buf()]);
    }

    if args.app.is_some() || args.window_name.is_some() || args.active_window {
        if let Some(app) = args.app.as_deref() {
            activate_app(app, system);
        }
        let windows = matching_windows(args)?;
        if windows.is_empty() {
            bail!("no matching window found; try --list-windows to inspect ids");
        }
        let suffixes: Vec<String> = windows
            .iter()
            .filter_map(|window| window.id().ok())
            .map(|id| format!("w{id}"))
            .collect();
        let paths = multi_output_paths(output, &suffixes)?;
        for (window, path) in windows.iter().zip(paths.iter()) {
            let image = window.capture_image().context("failed to capture window")?;
            save_image(&image, path, &args.format)?;
        }
        return Ok(paths);
    }

    if let Some(region) = args.region {
        let image = capture_region(region)?;
        save_image(&image, output, &args.format)?;
        return Ok(vec![output.to_path_buf()]);
    }

    capture_full_screen(output, &args.format, system)
}

fn list_windows(args: &Cli) -> Result<Vec<WindowMeta>> {
    let mut windows = window_metas()?;
    windows.retain(|window| window_matches(window, args));
    if args.active_window {
        windows.retain(|window| window.focused);
    }
    Ok(windows)
}

fn emit_window_list(windows: &[WindowMeta], json_output: bool) -> Result<()> {
    if json_output {
        let rows = windows.iter().map(window_meta_json).collect::<Vec<_>>();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema_version": "screenshot-rs-window-list-v1",
                "ok": true,
                "windows": rows,
            }))?
        );
        return Ok(());
    }

    if windows.is_empty() {
        println!("no matching windows found");
        return Ok(());
    }
    for window in windows {
        println!(
            "{}\t{}\t{}\t{}x{}+{}+{}",
            window.id, window.owner, window.title, window.width, window.height, window.x, window.y
        );
    }
    Ok(())
}

fn emit_capture_paths(paths: &[PathBuf], args: &Cli, system: &str) -> Result<()> {
    if args.json {
        let path_values = paths
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "schema_version": "screenshot-rs-capture-v1",
                "ok": true,
                "kind": capture_kind(args),
                "system": system,
                "format": normalized_format(&args.format),
                "paths": path_values,
            }))?
        );
        return Ok(());
    }

    for path in paths {
        println!("{}", path.display());
    }
    Ok(())
}

fn window_meta_json(window: &WindowMeta) -> serde_json::Value {
    json!({
        "id": window.id,
        "owner": window.owner,
        "title": window.title,
        "x": window.x,
        "y": window.y,
        "width": window.width,
        "height": window.height,
        "layer": window.layer,
        "focused": window.focused,
    })
}

fn capture_kind(args: &Cli) -> &'static str {
    if args.interactive {
        "interactive"
    } else if args.window_id.is_some()
        || args.active_window
        || args.app.is_some()
        || args.window_name.is_some()
    {
        "window"
    } else if args.region.is_some() {
        "region"
    } else {
        "screen"
    }
}

fn matching_windows(args: &Cli) -> Result<Vec<Window>> {
    let mut windows: Vec<(Window, WindowMeta)> = Window::all()
        .context("failed to list windows")?
        .into_iter()
        .filter_map(|window| window_meta(&window).ok().map(|meta| (window, meta)))
        .filter(|(_, meta)| window_matches(meta, args))
        .collect();

    if args.active_window {
        windows.retain(|(_, meta)| meta.focused);
    }
    windows.sort_by(window_order);

    if args.active_window {
        Ok(windows
            .into_iter()
            .take(1)
            .map(|(window, _)| window)
            .collect())
    } else {
        Ok(windows.into_iter().map(|(window, _)| window).collect())
    }
}

fn window_order(a: &(Window, WindowMeta), b: &(Window, WindowMeta)) -> std::cmp::Ordering {
    let a_layer_penalty = if a.1.layer == 0 { 0 } else { 1 };
    let b_layer_penalty = if b.1.layer == 0 { 0 } else { 1 };
    a_layer_penalty
        .cmp(&b_layer_penalty)
        .then_with(|| b.1.area().cmp(&a.1.area()))
}

fn find_window_by_id(id: u32) -> Result<Window> {
    Window::all()
        .context("failed to list windows")?
        .into_iter()
        .find(|window| window.id().ok() == Some(id))
        .ok_or_else(|| anyhow!("no matching window found for id {id}"))
}

fn window_metas() -> Result<Vec<WindowMeta>> {
    Ok(Window::all()
        .context("failed to list windows")?
        .iter()
        .filter_map(|window| window_meta(window).ok())
        .collect())
}

fn window_meta(window: &Window) -> Result<WindowMeta> {
    let width = window.width().unwrap_or(0);
    let height = window.height().unwrap_or(0);
    Ok(WindowMeta {
        id: window.id()?,
        owner: window.app_name().unwrap_or_default(),
        title: window.title().unwrap_or_default(),
        x: window.x().unwrap_or(0),
        y: window.y().unwrap_or(0),
        width,
        height,
        layer: window.z().unwrap_or(0),
        focused: window.is_focused().unwrap_or(false),
    })
}

impl WindowMeta {
    fn area(&self) -> u32 {
        self.width.saturating_mul(self.height)
    }
}

fn window_matches(window: &WindowMeta, args: &Cli) -> bool {
    if window.width == 0 || window.height == 0 {
        return false;
    }
    if let Some(app) = args.app.as_deref() {
        if !window
            .owner
            .to_ascii_lowercase()
            .contains(&app.to_ascii_lowercase())
        {
            return false;
        }
    }
    if let Some(name) = args.window_name.as_deref() {
        if !window
            .title
            .to_ascii_lowercase()
            .contains(&name.to_ascii_lowercase())
        {
            return false;
        }
    }
    true
}

fn capture_region(region: Region) -> Result<RgbaImage> {
    let monitors = Monitor::all().context("failed to list monitors")?;
    for monitor in monitors {
        let mx = monitor.x().unwrap_or(0);
        let my = monitor.y().unwrap_or(0);
        let mw = monitor.width().unwrap_or(0);
        let mh = monitor.height().unwrap_or(0);
        let right = mx.saturating_add(mw as i32);
        let bottom = my.saturating_add(mh as i32);
        let region_right = region.x.saturating_add(region.width as i32);
        let region_bottom = region.y.saturating_add(region.height as i32);
        if region.x >= mx && region.y >= my && region_right <= right && region_bottom <= bottom {
            let rel_x = (region.x - mx) as u32;
            let rel_y = (region.y - my) as u32;
            return monitor
                .capture_region(rel_x, rel_y, region.width, region.height)
                .context("failed to capture region");
        }
    }
    bail!("region must fit within a single monitor")
}

fn capture_full_screen(output: &Path, fmt: &str, system: &str) -> Result<Vec<PathBuf>> {
    let monitors = Monitor::all().context("failed to list monitors")?;
    if monitors.is_empty() {
        bail!("no monitors found");
    }

    if is_macos(system) && monitors.len() > 1 {
        let suffixes: Vec<String> = (1..=monitors.len()).map(|idx| format!("d{idx}")).collect();
        let paths = multi_output_paths(output, &suffixes)?;
        for (monitor, path) in monitors.iter().zip(paths.iter()) {
            let image = monitor
                .capture_image()
                .context("failed to capture monitor")?;
            save_image(&image, path, fmt)?;
        }
        return Ok(paths);
    }

    let image = if monitors.len() == 1 {
        monitors[0]
            .capture_image()
            .context("failed to capture monitor")?
    } else {
        composite_monitors(&monitors)?
    };
    save_image(&image, output, fmt)?;
    Ok(vec![output.to_path_buf()])
}

fn composite_monitors(monitors: &[Monitor]) -> Result<RgbaImage> {
    let mut captured = Vec::with_capacity(monitors.len());
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;

    for monitor in monitors {
        let x = monitor.x().unwrap_or(0);
        let y = monitor.y().unwrap_or(0);
        let image = monitor
            .capture_image()
            .context("failed to capture monitor")?;
        let width = image.width();
        let height = image.height();
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x.saturating_add(width as i32));
        max_y = max_y.max(y.saturating_add(height as i32));
        captured.push((x, y, image));
    }

    let width = (max_x - min_x) as u32;
    let height = (max_y - min_y) as u32;
    let mut canvas: RgbaImage = ImageBuffer::from_pixel(width, height, Rgba([0, 0, 0, 0]));
    for (x, y, image) in captured {
        imageops::overlay(&mut canvas, &image, (x - min_x) as i64, (y - min_y) as i64);
    }
    Ok(canvas)
}

fn save_image(image: &RgbaImage, output: &Path, fmt: &str) -> Result<()> {
    ensure_parent(output)?;
    let format = image_format(fmt)?;
    let dynamic = if matches!(format, ImageFormat::Jpeg) {
        DynamicImage::ImageRgb8(DynamicImage::ImageRgba8(image.clone()).to_rgb8())
    } else {
        DynamicImage::ImageRgba8(image.clone())
    };
    dynamic
        .save_with_format(output, format)
        .with_context(|| format!("failed to save image: {}", output.display()))
}

fn image_format(fmt: &str) -> Result<ImageFormat> {
    match normalized_format(fmt).as_str() {
        "png" => Ok(ImageFormat::Png),
        "jpg" | "jpeg" => Ok(ImageFormat::Jpeg),
        "bmp" => Ok(ImageFormat::Bmp),
        "gif" => Ok(ImageFormat::Gif),
        "webp" => Ok(ImageFormat::WebP),
        other => bail!("unsupported format: {other}"),
    }
}

fn write_test_image(path: &Path, fmt: &str) -> Result<()> {
    let image = ImageBuffer::from_pixel(1, 1, Rgba([255, 255, 255, 255]));
    save_image(&image, path, fmt)
}

fn capture_interactive(args: &Cli, output: &Path, system: &str) -> Result<()> {
    if !is_macos(system) {
        bail!("--interactive is supported on macOS only");
    }
    let status = Command::new("screencapture")
        .arg("-x")
        .arg(format!("-t{}", normalized_format(&args.format)))
        .arg("-i")
        .arg(output)
        .status()
        .context("required command not found: screencapture")?;
    if !status.success() {
        bail!("command failed: screencapture");
    }
    Ok(())
}

fn activate_app(app: &str, system: &str) {
    if !is_macos(system) {
        return;
    }
    let escaped = app.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!("tell application \"{escaped}\" to activate");
    let _ = Command::new("osascript")
        .args(["-e", &script])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

fn is_macos(system: &str) -> bool {
    system.eq_ignore_ascii_case("macos") || system.eq_ignore_ascii_case("darwin")
}

#[cfg(target_os = "macos")]
fn ensure_macos_permissions() -> Result<()> {
    if env::var_os("CODEX_SANDBOX").is_some() {
        bail!("screen capture checks are blocked in the sandbox; rerun outside the sandbox");
    }

    let access = core_graphics::access::ScreenCaptureAccess;
    if access.preflight() {
        println!("Screen Recording permission already granted.");
        return Ok(());
    }

    eprintln!(
        "This workflow needs macOS Screen Recording permission to capture screenshots.\n\
         Approve the system prompt, or enable Screen Recording for your terminal in System Settings."
    );
    let _ = access.request();
    if !access.preflight() {
        bail!(
            "Screen Recording permission is still not granted; enable it in System Settings > Privacy & Security > Screen Recording and retry"
        );
    }
    println!("Screen Recording permission granted.");
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn ensure_macos_permissions() -> Result<()> {
    bail!("--ensure-macos-permissions only supports macOS")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_region_accepts_valid_input() {
        assert_eq!(
            parse_region("100,200,800,600").unwrap(),
            Region {
                x: 100,
                y: 200,
                width: 800,
                height: 600
            }
        );
    }

    #[test]
    fn parse_region_rejects_bad_input() {
        assert!(parse_region("1,2,3").is_err());
        assert!(parse_region("1,2,0,4").is_err());
        assert!(parse_region("1,2,x,4").is_err());
    }

    #[test]
    fn multi_output_paths_adds_suffixes_before_extension() {
        let paths = multi_output_paths(
            Path::new("/tmp/screen.png"),
            &["d1".to_string(), "d2".to_string()],
        )
        .unwrap();
        assert_eq!(paths[0], PathBuf::from("/tmp/screen-d1.png"));
        assert_eq!(paths[1], PathBuf::from("/tmp/screen-d2.png"));
    }

    #[test]
    fn requested_path_without_extension_gets_format() {
        let path =
            resolve_output_path(Some("/tmp/codex-shot-test"), "default", "png", "linux").unwrap();
        assert_eq!(path, PathBuf::from("/tmp/codex-shot-test.png"));
    }
}
