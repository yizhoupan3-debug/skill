use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use image::{imageops::FilterType};
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
        Commands::Resize { input, width, height, scale, output } => {
            let img = image::open(&input)?;
            let out_path = output.clone().unwrap_or_else(|| auto_output(&input, "_resized"));
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
            println!("OK: {} -> {} ({}x{})", input, out_path, resized.width(), resized.height());
        }
        Commands::Crop { input, box_coords, output } => {
            let mut img = image::open(&input)?;
            let parts: Vec<&str> = box_coords.split(',').collect();
            if parts.len() != 4 {
                return Err(anyhow!("--box_coords must be left,upper,right,lower"));
            }
            let left: u32 = parts[0].parse()?;
            let upper: u32 = parts[1].parse()?;
            let right: u32 = parts[2].parse()?;
            let lower: u32 = parts[3].parse()?;
            
            let cropped = img.crop(left, upper, right - left, lower - upper);
            let out_path = output.unwrap_or_else(|| auto_output(&input, "_cropped"));
            cropped.save(&out_path)?;
            println!("OK: {} -> {} ({}x{})", input, out_path, cropped.width(), cropped.height());
        }
        Commands::Convert { input, format, quality: _, output } => {
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
        Commands::Info { input } => {
            let img = image::open(&input)?;
            println!("File:   {}", input);
            println!("Size:   {}x{}", img.width(), img.height());
            println!("Color:  {:?}", img.color());
        }
    }

    Ok(())
}
