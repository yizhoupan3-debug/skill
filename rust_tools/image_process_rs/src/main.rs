use anyhow::{anyhow, bail, Result};
use clap::{Parser, Subcommand};
use image::{imageops::FilterType, DynamicImage, ImageBuffer, Rgba};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(author, version, about = "Non-AI image processing CLI in Rust", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Resize image
    Resize {
        input: String,
        #[arg(long)]
        width: Option<u32>,
        #[arg(long)]
        height: Option<u32>,
        #[arg(long)]
        scale: Option<f32>,
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Crop image
    Crop {
        input: String,
        /// left,upper,right,lower (e.g., "100,100,500,400")
        #[arg(long)]
        box_coords: String,
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Convert format
    Convert {
        input: String,
        /// webp, jpeg, png, etc.
        #[arg(long)]
        format: String,
        #[arg(long)]
        quality: Option<u8>,
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Enhance brightness, contrast, color, sharpness, and denoise
    Enhance {
        input: String,
        #[arg(long, default_value_t = 1.0)]
        brightness: f32,
        #[arg(long, default_value_t = 1.0)]
        contrast: f32,
        #[arg(long, default_value_t = 1.0)]
        color: f32,
        #[arg(long, default_value_t = 1.0)]
        sharpness: f32,
        #[arg(long, default_value_t = 0.0)]
        denoise: f32,
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Add a simple bitmap text watermark
    Watermark {
        input: String,
        #[arg(long)]
        text: String,
        #[arg(long, default_value_t = 36)]
        size: u32,
        #[arg(long, default_value_t = 0.5)]
        opacity: f32,
        #[arg(long)]
        x: Option<u32>,
        #[arg(long)]
        y: Option<u32>,
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Show info
    Info { input: String },
}

fn auto_output(input: &str, suffix: &str) -> String {
    let p = Path::new(input);
    let stem = p.file_stem().unwrap_or_default().to_string_lossy();
    let parent = p.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
    let ext = p.extension().unwrap_or_default().to_string_lossy();
    let mut out = parent.join(format!("{}{}", stem, suffix));
    if !ext.is_empty() {
        out.set_extension(ext.as_ref());
    }
    out.to_string_lossy().to_string()
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Resize {
            input,
            width,
            height,
            scale,
            output,
        } => {
            let img = image::open(&input)?;
            let out_path = output
                .clone()
                .unwrap_or_else(|| auto_output(&input, "_resized"));
            let resized = if let Some(s) = scale {
                let w = (img.width() as f32 * s) as u32;
                let h = (img.height() as f32 * s) as u32;
                img.resize(w, h, FilterType::Lanczos3)
            } else if let (Some(w), Some(h)) = (width, height) {
                img.resize_exact(w, h, FilterType::Lanczos3)
            } else if let Some(w) = width {
                let ratio = w as f32 / img.width() as f32;
                let h = (img.height() as f32 * ratio) as u32;
                img.resize(w, h, FilterType::Lanczos3)
            } else if let Some(h) = height {
                let ratio = h as f32 / img.height() as f32;
                let w = (img.width() as f32 * ratio) as u32;
                img.resize(w, h, FilterType::Lanczos3)
            } else {
                return Err(anyhow!("Must specify width, height, or scale"));
            };
            resized.save(&out_path)?;
            println!(
                "OK: {} -> {} ({}x{})",
                input,
                out_path,
                resized.width(),
                resized.height()
            );
        }
        Commands::Crop {
            input,
            box_coords,
            output,
        } => {
            let mut img = image::open(&input)?;
            let parts: Vec<&str> = box_coords.split(',').collect();
            if parts.len() != 4 {
                return Err(anyhow!("--box_coords must be left,upper,right,lower"));
            }
            let left: u32 = parts[0].parse()?;
            let upper: u32 = parts[1].parse()?;
            let right: u32 = parts[2].parse()?;
            let lower: u32 = parts[3].parse()?;
            if right <= left || lower <= upper {
                bail!("crop box must satisfy right > left and lower > upper");
            }

            let cropped = img.crop(left, upper, right - left, lower - upper);
            let out_path = output.unwrap_or_else(|| auto_output(&input, "_cropped"));
            cropped.save(&out_path)?;
            println!(
                "OK: {} -> {} ({}x{})",
                input,
                out_path,
                cropped.width(),
                cropped.height()
            );
        }
        Commands::Convert {
            input,
            format,
            quality: _,
            output,
        } => {
            let img = image::open(&input)?;
            let out_path = if let Some(o) = output {
                PathBuf::from(o)
            } else {
                let p = Path::new(&input);
                let ext = format.to_lowercase();
                let parent = p.parent().unwrap_or_else(|| Path::new("")).to_path_buf();
                let stem = p.file_stem().unwrap_or_default().to_string_lossy();
                let mut out = parent.join(format!("{}", stem));
                let real_ext = if ext == "jpg" { "jpeg" } else { &ext };
                out.set_extension(real_ext);
                out
            };
            // Note: Currently ignoring quality arg for simplicity in base version
            img.save(&out_path)?;
            println!("OK: {} -> {}", input, out_path.display());
        }
        Commands::Enhance {
            input,
            brightness,
            contrast,
            color,
            sharpness,
            denoise,
            output,
        } => {
            let img = image::open(&input)?;
            let enhanced = enhance_image(img, brightness, contrast, color, sharpness, denoise)?;
            let out_path = output.unwrap_or_else(|| auto_output(&input, "_enhanced"));
            enhanced.save(&out_path)?;
            println!("OK: {} -> {}", input, out_path);
        }
        Commands::Watermark {
            input,
            text,
            size,
            opacity,
            x,
            y,
            output,
        } => {
            if !(0.0..=1.0).contains(&opacity) {
                bail!("--opacity must be between 0.0 and 1.0");
            }
            let mut img = image::open(&input)?.to_rgba8();
            let x = x.unwrap_or(10);
            let y = y.unwrap_or_else(|| img.height().saturating_sub(size + 10));
            draw_watermark(&mut img, &text, x, y, size, opacity);
            let out_path = output.unwrap_or_else(|| auto_output(&input, "_watermarked"));
            DynamicImage::ImageRgba8(img).save(&out_path)?;
            println!("OK: {} -> {}", input, out_path);
        }
        Commands::Info { input } => {
            let img = image::open(&input)?;
            println!("File:   {}", input);
            println!("Size:   {}x{}", img.width(), img.height());
            println!("Color:  {:?}", img.color());
        }
    }

    Ok(())
}

fn enhance_image(
    img: DynamicImage,
    brightness: f32,
    contrast: f32,
    color: f32,
    sharpness: f32,
    denoise: f32,
) -> Result<DynamicImage> {
    if brightness <= 0.0 || contrast <= 0.0 || color <= 0.0 || sharpness <= 0.0 {
        bail!("enhancement factors must be > 0");
    }
    let mut out = img;
    if (brightness - 1.0).abs() > f32::EPSILON {
        out = out.brighten(((brightness - 1.0) * 255.0).round() as i32);
    }
    if (contrast - 1.0).abs() > f32::EPSILON {
        out = out.adjust_contrast((contrast - 1.0) * 100.0);
    }
    if (color - 1.0).abs() > f32::EPSILON {
        out = adjust_saturation(out, color);
    }
    if (sharpness - 1.0).abs() > f32::EPSILON {
        out = if sharpness > 1.0 {
            out.unsharpen(1.0, ((sharpness - 1.0) * 8.0).max(1.0) as i32)
        } else {
            out.blur((1.0 - sharpness) * 2.0)
        };
    }
    if denoise > 0.0 {
        out = out.blur(denoise);
    }
    Ok(out)
}

fn adjust_saturation(img: DynamicImage, factor: f32) -> DynamicImage {
    let mut rgba = img.to_rgba8();
    for pixel in rgba.pixels_mut() {
        let [r, g, b, a] = pixel.0;
        let rf = r as f32;
        let gf = g as f32;
        let bf = b as f32;
        let gray = 0.299 * rf + 0.587 * gf + 0.114 * bf;
        pixel.0 = [
            clamp_u8(gray + (rf - gray) * factor),
            clamp_u8(gray + (gf - gray) * factor),
            clamp_u8(gray + (bf - gray) * factor),
            a,
        ];
    }
    DynamicImage::ImageRgba8(rgba)
}

fn clamp_u8(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

fn draw_watermark(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    text: &str,
    x: u32,
    y: u32,
    size: u32,
    opacity: f32,
) {
    let scale = (size / 8).max(1);
    let alpha = (255.0 * opacity).round().clamp(0.0, 255.0) as u8;
    let mut cursor_x = x;
    for ch in text.chars() {
        draw_char_block(img, ch, cursor_x, y, scale, alpha);
        cursor_x = cursor_x.saturating_add(6 * scale);
    }
}

fn draw_char_block(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    ch: char,
    x: u32,
    y: u32,
    scale: u32,
    alpha: u8,
) {
    let pattern = simple_glyph(ch);
    for (row, bits) in pattern.iter().enumerate() {
        for col in 0..5 {
            if bits & (1 << (4 - col)) == 0 {
                continue;
            }
            fill_rect(
                img,
                x + col * scale,
                y + row as u32 * scale,
                scale,
                scale,
                Rgba([255, 255, 255, alpha]),
            );
        }
    }
}

fn fill_rect(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: Rgba<u8>,
) {
    for yy in y..y.saturating_add(height).min(img.height()) {
        for xx in x..x.saturating_add(width).min(img.width()) {
            let base = *img.get_pixel(xx, yy);
            img.put_pixel(xx, yy, alpha_blend(base, color));
        }
    }
}

fn alpha_blend(base: Rgba<u8>, overlay: Rgba<u8>) -> Rgba<u8> {
    let alpha = overlay[3] as f32 / 255.0;
    let inv = 1.0 - alpha;
    Rgba([
        clamp_u8(base[0] as f32 * inv + overlay[0] as f32 * alpha),
        clamp_u8(base[1] as f32 * inv + overlay[1] as f32 * alpha),
        clamp_u8(base[2] as f32 * inv + overlay[2] as f32 * alpha),
        base[3],
    ])
}

fn simple_glyph(ch: char) -> [u8; 7] {
    match ch.to_ascii_uppercase() {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [15, 16, 16, 16, 16, 16, 15],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [15, 16, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [31, 4, 4, 4, 4, 4, 31],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 10, 10, 4],
        'W' => [17, 17, 17, 21, 21, 27, 17],
        'X' => [17, 10, 4, 4, 4, 10, 17],
        'Y' => [17, 10, 4, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [15, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 30],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        '_' => [0, 0, 0, 0, 0, 0, 31],
        '.' => [0, 0, 0, 0, 0, 12, 12],
        ':' => [0, 12, 12, 0, 12, 12, 0],
        '/' => [1, 2, 2, 4, 8, 8, 16],
        ' ' => [0; 7],
        _ => [31, 17, 6, 6, 4, 0, 4],
    }
}
