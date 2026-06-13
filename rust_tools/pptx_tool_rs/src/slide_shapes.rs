// Extracted from lib.rs: slide_shapes.



/// Configuration for data-driven slide layout rendering.
/// Replaces 5 nearly-identical `slide_shapes_*` functions with a single generic renderer.
#[derive(Debug, Clone)]
struct LayoutConfig {
    /// Panel name (e.g. "Content Panel", "Hero Panel")
    panel_name: &'static str,
    /// Panel bounding box
    panel_box: ShapeBox,
    /// Panel fill color field: true = palette.panel, false = palette.panel_soft
    panel_use_hard: bool,
    /// Panel z-order
    panel_z: u32,
    /// Title bounding box
    title_box: ShapeBox,
    /// Title font size
    title_size: u32,
    /// Subtitle bounding box (if subtitle exists)
    subtitle_box: ShapeBox,
    /// Body item start y coordinate
    body_start_y: f64,
    /// Body item y step
    body_step_y: f64,
    /// Body item bounding box width/height
    body_w: f64,
    body_h: f64,
    /// Body item x offset
    body_x: f64,
    /// Maximum body items
    body_max: usize,
    /// Whether to number body items (1. 2. 3.)
    body_numbered: bool,
}

/// Generic data-driven slide shape renderer.
/// Replaces `slide_shapes_cover_or_closing`, `slide_shapes_standard_content`,
/// `slide_shapes_data_panel`, `slide_shapes_step_lane`, `slide_shapes_multi_card`.
fn slide_shapes_from_config(slide: &PptxSlideSpec, palette: &PptPalette, cfg: &LayoutConfig) -> Vec<String> {
    let panel_color = if cfg.panel_use_hard { palette.panel } else { palette.panel_soft };
    let panel = rect_shape(4, cfg.panel_name, cfg.panel_box.clone(), panel_color, Some(palette.line), cfg.panel_z);
    let mut shapes = vec![panel, slide_shapes_label(slide, palette)];
    shapes.push(text_shape(11, "Title", &slide.title, cfg.title_box.clone(), TextStyle {
        size_pt: cfg.title_size, color: palette.text, bold: true, title_placeholder: true,
    }));
    if !slide.subtitle.is_empty() {
        shapes.push(text_shape(12, "Subtitle", &slide.subtitle, cfg.subtitle_box.clone(), TextStyle {
            size_pt: 18, color: palette.text_soft, bold: false, title_placeholder: false,
        }));
    }
    for (idx, item) in slide.body.iter().take(cfg.body_max).enumerate() {
        let y = cfg.body_start_y + idx as f64 * cfg.body_step_y;
        let text = if cfg.body_numbered { format!("{}. {}", idx + 1, item) } else { item.clone() };
        shapes.push(text_shape(20 + idx as u32, "Body", &text, ShapeBox {
            x: cfg.body_x, y, w: cfg.body_w, h: cfg.body_h,
        }, TextStyle {
            size_pt: 18, color: palette.text_soft, bold: false, title_placeholder: false,
        }));
    }
    shapes
}

pub fn slide_shapes_cover_or_closing(slide: &PptxSlideSpec, palette: &PptPalette) -> Vec<String> {
    // cover vs closing: title/subtitle positions differ
    let (title_box, title_size, subtitle_y) = if slide.layout == "cover" {
        (ShapeBox { x: 0.96, y: 2.02, w: 4.9, h: 1.1 }, 31u32, 3.2)
    } else {
        (ShapeBox { x: 3.2, y: 2.48, w: 6.9, h: 0.9 }, 34, 3.52)
    };
    slide_shapes_from_config(slide, palette, &LayoutConfig {
        panel_name: "Hero Panel",
        panel_box: ShapeBox { x: 0.72, y: 0.72, w: 5.8, h: 6.0 },
        panel_use_hard: true, panel_z: 16000,
        title_box, title_size,
        subtitle_box: ShapeBox { x: 0.96, y: subtitle_y, w: 6.7, h: 0.38 },
        body_start_y: 4.35, body_step_y: 0.58,
        body_x: 1.18, body_w: 10.9, body_h: 0.42,
        body_max: 6, body_numbered: false,
    })
}

pub fn slide_shapes_standard_content(slide: &PptxSlideSpec, palette: &PptPalette) -> Vec<String> {
    slide_shapes_from_config(slide, palette, &LayoutConfig {
        panel_name: "Content Panel",
        panel_box: ShapeBox { x: 0.86, y: 1.62, w: 11.6, h: 4.82 },
        panel_use_hard: false, panel_z: 10000,
        title_box: ShapeBox { x: 0.92, y: 0.92, w: 6.3, h: 0.6 }, title_size: 24,
        subtitle_box: ShapeBox { x: 0.96, y: 1.56, w: 6.7, h: 0.38 },
        body_start_y: 2.0, body_step_y: 0.58,
        body_x: 1.18, body_w: 10.9, body_h: 0.42,
        body_max: 6, body_numbered: true,
    })
}

pub fn slide_shapes_comparison(slide: &PptxSlideSpec, palette: &PptPalette) -> Vec<String> {
    let left_panel = rect_shape(
        4,
        "Compare Left",
        ShapeBox {
            x: 0.86,
            y: 1.62,
            w: 5.65,
            h: 4.82,
        },
        palette.panel_soft,
        Some(palette.line),
        10000,
    );
    let right_panel = rect_shape(
        5,
        "Compare Right",
        ShapeBox {
            x: 6.72,
            y: 1.62,
            w: 5.74,
            h: 4.82,
        },
        palette.panel_soft,
        Some(palette.line),
        10000,
    );
    let mut shapes = vec![left_panel, right_panel, slide_shapes_label(slide, palette)];
    shapes.push(text_shape(
        11,
        "Title",
        &slide.title,
        ShapeBox {
            x: 0.92,
            y: 0.92,
            w: 11.2,
            h: 0.6,
        },
        TextStyle {
            size_pt: 24,
            color: palette.text,
            bold: true,
            title_placeholder: true,
        },
    ));
    let left_txt = slide.body.first().cloned().unwrap_or_default();
    let right_txt = slide.body.get(1).cloned().unwrap_or_default();
    shapes.push(text_shape(
        22,
        "Column A",
        &left_txt,
        ShapeBox {
            x: 1.08,
            y: 2.05,
            w: 5.2,
            h: 4.2,
        },
        TextStyle {
            size_pt: 17,
            color: palette.text_soft,
            bold: false,
            title_placeholder: false,
        },
    ));
    shapes.push(text_shape(
        23,
        "Column B",
        &right_txt,
        ShapeBox {
            x: 6.88,
            y: 2.05,
            w: 5.45,
            h: 4.2,
        },
        TextStyle {
            size_pt: 17,
            color: palette.text_soft,
            bold: false,
            title_placeholder: false,
        },
    ));
    shapes
}

pub fn slide_shapes_hero_image(slide: &PptxSlideSpec, palette: &PptPalette) -> Vec<String> {
    let image_plate = rect_shape(
        4,
        "Image Plate",
        ShapeBox {
            x: 0.86,
            y: 1.62,
            w: 5.25,
            h: 4.82,
        },
        palette.panel,
        Some(palette.line),
        9000,
    );
    let text_panel = rect_shape(
        5,
        "Text Panel",
        ShapeBox {
            x: 6.35,
            y: 1.62,
            w: 6.1,
            h: 4.82,
        },
        palette.panel_soft,
        Some(palette.line),
        10000,
    );
    let mut shapes = vec![image_plate, text_panel, slide_shapes_label(slide, palette)];
    shapes.push(text_shape(
        21,
        "Image Hint",
        "Image / schematic",
        ShapeBox {
            x: 1.1,
            y: 3.35,
            w: 4.7,
            h: 0.45,
        },
        TextStyle {
            size_pt: 14,
            color: palette.text_mute,
            bold: false,
            title_placeholder: false,
        },
    ));
    shapes.push(text_shape(
        11,
        "Title",
        &slide.title,
        ShapeBox {
            x: 0.92,
            y: 0.92,
            w: 11.35,
            h: 0.58,
        },
        TextStyle {
            size_pt: 24,
            color: palette.text,
            bold: true,
            title_placeholder: true,
        },
    ));
    if !slide.subtitle.is_empty() {
        shapes.push(text_shape(
            12,
            "Subtitle",
            &slide.subtitle,
            ShapeBox {
                x: 0.92,
                y: 1.48,
                w: 10.9,
                h: 0.35,
            },
            TextStyle {
                size_pt: 16,
                color: palette.text_soft,
                bold: false,
                title_placeholder: false,
            },
        ));
    }
    for (idx, item) in slide.body.iter().take(6).enumerate() {
        let y = 2.08 + idx as f64 * 0.62;
        let text = format!("{}. {}", idx + 1, item);
        shapes.push(text_shape(
            31 + idx as u32,
            "Body",
            &text,
            ShapeBox {
                x: 6.55,
                y,
                w: 5.75,
                h: 0.48,
            },
            TextStyle {
                size_pt: 17,
                color: palette.text_soft,
                bold: false,
                title_placeholder: false,
            },
        ));
    }
    shapes
}

pub fn slide_shapes_image_text_split(slide: &PptxSlideSpec, palette: &PptPalette) -> Vec<String> {
    let top_band = rect_shape(
        4,
        "Visual Band",
        ShapeBox {
            x: 0.86,
            y: 1.62,
            w: 11.6,
            h: 2.65,
        },
        palette.panel,
        Some(palette.line),
        9500,
    );
    let lower_panel = rect_shape(
        5,
        "Lower Panel",
        ShapeBox {
            x: 0.86,
            y: 4.55,
            w: 11.6,
            h: 1.88,
        },
        palette.panel_soft,
        Some(palette.line),
        10000,
    );
    let mut shapes = vec![top_band, lower_panel, slide_shapes_label(slide, palette)];
    shapes.push(text_shape(
        11,
        "Title",
        &slide.title,
        ShapeBox {
            x: 0.92,
            y: 0.92,
            w: 11.2,
            h: 0.55,
        },
        TextStyle {
            size_pt: 22,
            color: palette.text,
            bold: true,
            title_placeholder: true,
        },
    ));
    shapes.push(text_shape(
        22,
        "Band Hint",
        "Figure / screenshot / diagram",
        ShapeBox {
            x: 1.08,
            y: 3.42,
            w: 10.8,
            h: 0.35,
        },
        TextStyle {
            size_pt: 13,
            color: palette.text_mute,
            bold: false,
            title_placeholder: false,
        },
    ));
    for (idx, item) in slide.body.iter().take(6).enumerate() {
        let y = 4.76 + idx as f64 * 0.42;
        let text = format!("{}. {}", idx + 1, item);
        shapes.push(text_shape(
            30 + idx as u32,
            "Body",
            &text,
            ShapeBox {
                x: 1.08,
                y,
                w: 11.1,
                h: 0.38,
            },
            TextStyle {
                size_pt: 16,
                color: palette.text_soft,
                bold: false,
                title_placeholder: false,
            },
        ));
    }
    shapes
}

pub fn slide_shapes_data_panel(slide: &PptxSlideSpec, palette: &PptPalette) -> Vec<String> {
    let panel = rect_shape(
        4,
        "Content Panel",
        ShapeBox {
            x: 0.86,
            y: 1.62,
            w: 11.6,
            h: 4.82,
        },
        palette.panel_soft,
        Some(palette.line),
        10000,
    );
    let mut shapes = vec![panel, slide_shapes_label(slide, palette)];
    shapes.push(text_shape(
        11,
        "Title",
        &slide.title,
        ShapeBox {
            x: 0.92,
            y: 0.92,
            w: 11.25,
            h: 0.62,
        },
        TextStyle {
            size_pt: 24,
            color: palette.text,
            bold: true,
            title_placeholder: true,
        },
    ));
    let kpi_count = slide.body.len().min(3);
    let col_w = 3.55;
    let gap = 0.38;
    let start_x = 1.05;
    for idx in 0..kpi_count {
        let x = start_x + idx as f64 * (col_w + gap);
        let line = slide
            .body
            .get(idx)
            .map(String::as_str)
            .unwrap_or("")
            .to_string();
        shapes.push(text_shape(
            50 + idx as u32,
            "Metric",
            &line,
            ShapeBox {
                x,
                y: 2.1,
                w: col_w,
                h: 1.25,
            },
            TextStyle {
                size_pt: 22,
                color: palette.text,
                bold: true,
                title_placeholder: false,
            },
        ));
    }
    let remainder_start = slide.body.len().min(3);
    for (r_idx, item) in slide.body.iter().skip(remainder_start).take(6).enumerate() {
        let y = 3.72 + r_idx as f64 * 0.52;
        let text = format!("{}. {}", r_idx + 1 + remainder_start, item);
        shapes.push(text_shape(
            60 + r_idx as u32,
            "Detail",
            &text,
            ShapeBox {
                x: 1.12,
                y,
                w: 10.95,
                h: 0.44,
            },
            TextStyle {
                size_pt: 17,
                color: palette.text_soft,
                bold: false,
                title_placeholder: false,
            },
        ));
    }
    shapes
}

pub fn slide_shapes_step_lane(slide: &PptxSlideSpec, palette: &PptPalette) -> Vec<String> {
    let panel = rect_shape(
        4,
        "Content Panel",
        ShapeBox {
            x: 0.86,
            y: 1.62,
            w: 11.6,
            h: 4.82,
        },
        palette.panel_soft,
        Some(palette.line),
        10000,
    );
    let mut shapes = vec![panel, slide_shapes_label(slide, palette)];
    shapes.push(text_shape(
        11,
        "Title",
        &slide.title,
        ShapeBox {
            x: 0.92,
            y: 0.92,
            w: 10.95,
            h: 0.6,
        },
        TextStyle {
            size_pt: 24,
            color: palette.text,
            bold: true,
            title_placeholder: true,
        },
    ));
    for (idx, item) in slide.body.iter().take(6).enumerate() {
        let y = 2.06 + idx as f64 * 0.76;
        shapes.push(rect_shape(
            70 + idx as u32,
            "Lane Accent",
            ShapeBox {
                x: 1.06,
                y: y + 0.06,
                w: 0.085,
                h: 0.55,
            },
            palette.glow,
            None,
            52000,
        ));
        shapes.push(text_shape(
            20 + idx as u32,
            "Step",
            &format!("{}. {}", idx + 1, item),
            ShapeBox {
                x: 1.25,
                y,
                w: 10.75,
                h: 0.65,
            },
            TextStyle {
                size_pt: 17,
                color: palette.text_soft,
                bold: false,
                title_placeholder: false,
            },
        ));
    }
    shapes
}

pub fn slide_shapes_multi_card(slide: &PptxSlideSpec, palette: &PptPalette) -> Vec<String> {
    let placements = [
        (0.92f64, 2.03f64, 5.68f64, 1.85f64),
        (6.65f64, 2.03f64, 5.68f64, 1.85f64),
        (0.92f64, 4.15f64, 5.68f64, 1.85f64),
        (6.65f64, 4.15f64, 5.68f64, 1.85f64),
    ];
    let mut shapes = vec![slide_shapes_label(slide, palette)];
    shapes.push(text_shape(
        11,
        "Title",
        &slide.title,
        ShapeBox {
            x: 0.92,
            y: 0.92,
            w: 10.95,
            h: 0.62,
        },
        TextStyle {
            size_pt: 24,
            color: palette.text,
            bold: true,
            title_placeholder: true,
        },
    ));
    let base_id: u32 = 80;
    for (idx, item) in slide.body.iter().take(4).enumerate() {
        let (cx, cy, cw, ch) = placements[idx];
        shapes.push(rect_shape(
            base_id + idx as u32 * 10,
            "Card",
            ShapeBox {
                x: cx - 0.06,
                y: cy - 0.12,
                w: cw + 0.12,
                h: ch + 0.16,
            },
            palette.panel_soft,
            Some(palette.line),
            11000,
        ));
        shapes.push(text_shape(
            base_id + idx as u32 * 10 + 1,
            "Card Copy",
            &format!("{}. {}", idx + 1, item),
            ShapeBox {
                x: cx + 0.12,
                y: cy + 0.1,
                w: cw - 0.26,
                h: ch - 0.2,
            },
            TextStyle {
                size_pt: 16,
                color: palette.text_soft,
                bold: false,
                title_placeholder: false,
            },
        ));
    }
    shapes
}

pub fn slide_shapes_page_footer(slide_no: usize, total: usize, palette: &PptPalette) -> String {
    text_shape(
        40,
        "Page",
        &format!("{slide_no:02} / {total:02}"),
        ShapeBox {
            x: 11.82,
            y: 7.0,
            w: 0.8,
            h: 0.22,
        },
        TextStyle {
            size_pt: 9,
            color: palette.text_mute,
            bold: false,
            title_placeholder: false,
        },
    )
}
