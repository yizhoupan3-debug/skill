// Extracted from lib.rs: pptx_extract_qa.


pub fn extract_pptx_structure(
    bundle: &ZipBundle,
    input: &Path,
    extract_images: bool,
    image_dir: Option<PathBuf>,
) -> Result<Value> {
    if let Some(dir) = &image_dir {
        if extract_images {
            fs::create_dir_all(dir)?;
        }
    }
    let presentation_xml = bundle.text("ppt/presentation.xml")?;
    let presentation_doc = Document::parse(&presentation_xml)?;
    let (slide_width, slide_height) = presentation_doc
        .descendants()
        .find(|node| node.tag_name().name() == "sldSz")
        .map(|node| {
            (
                attr_f64(&node, "cx").unwrap_or_default() / EMU_PER_INCH,
                attr_f64(&node, "cy").unwrap_or_default() / EMU_PER_INCH,
            )
        })
        .ok_or_else(|| anyhow!("missing slide size in presentation.xml"))?;

    let presentation_rels = parse_relationships(&bundle.text("ppt/_rels/presentation.xml.rels")?)?;
    let slide_refs = presentation_doc
        .descendants()
        .filter(|node| node.tag_name().name() == "sldId")
        .filter_map(|node| rel_attr_value(&node, "id").map(str::to_string))
        .collect::<Vec<_>>();

    let mut slides = Vec::new();
    for (idx, rel_id) in slide_refs.iter().enumerate() {
        let rel_target = presentation_rels
            .get(rel_id)
            .ok_or_else(|| anyhow!("missing relationship {} in presentation rels", rel_id))?;
        let slide_path = normalize_zip_path(&format!("ppt/{}", rel_target));
        let slide_xml = bundle.text(&slide_path)?;
        let slide_doc = Document::parse(&slide_xml)?;
        let rel_path = slide_rel_path(&slide_path);
        let slide_rels = bundle
            .text(&rel_path)
            .ok()
            .map(|text| parse_relationships(&text))
            .transpose()?
            .unwrap_or_default();
        let layout_name = slide_rels
            .iter()
            .find(|(_, target)| target.contains("slideLayouts"))
            .and_then(|(_, target)| extract_layout_name(bundle, target).ok());
        let notes = slide_rels
            .iter()
            .find(|(_, target)| target.contains("notesSlides"))
            .and_then(|(_, target)| extract_notes(bundle, target).ok())
            .filter(|text| !text.trim().is_empty());
        let elements = extract_slide_elements(
            bundle,
            &slide_doc,
            &slide_rels,
            idx,
            extract_images,
            image_dir.as_deref(),
        )?;
        slides.push(json!({
            "index": idx,
            "layout": layout_name,
            "elements": elements,
            "notes": notes,
        }));
    }

    let available_layouts = bundle
        .names()
        .filter(|name| name.starts_with("ppt/slideLayouts/slideLayout") && name.ends_with(".xml"))
        .filter_map(|name| extract_layout_info(bundle, name).ok())
        .collect::<Vec<_>>();

    Ok(json!({
        "file": input.file_name().and_then(OsStr::to_str).unwrap_or_default(),
        "slide_width": round4(slide_width),
        "slide_height": round4(slide_height),
        "slide_count": slides.len(),
        "slides": slides,
        "available_layouts": available_layouts,
    }))
}

pub fn slide_rel_path(slide_path: &str) -> String {
    let path = Path::new(slide_path);
    let file_name = path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("slide1.xml");
    let parent = path.parent().unwrap_or_else(|| Path::new("ppt/slides"));
    normalize_zip_path(
        &parent
            .join("_rels")
            .join(format!("{}.rels", file_name))
            .display()
            .to_string(),
    )
}

pub fn parse_relationships(xml: &str) -> Result<HashMap<String, String>> {
    let doc = Document::parse(xml)?;
    let mut rels = HashMap::new();
    for node in doc
        .descendants()
        .filter(|node| node.tag_name().name() == "Relationship")
    {
        if let (Some(id), Some(target)) = (attr_value(&node, "Id"), attr_value(&node, "Target")) {
            rels.insert(id.to_string(), target.to_string());
        }
    }
    Ok(rels)
}

pub fn resolve_target(base: &str, target: &str) -> String {
    let base_path = Path::new(base);
    let joined = if target.starts_with("../") || target.starts_with("./") {
        base_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(target)
    } else if target.starts_with('/') {
        PathBuf::from(target.trim_start_matches('/'))
    } else {
        base_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(target)
    };
    normalize_path_like_zip(&joined)
}

pub fn extract_layout_name(bundle: &ZipBundle, rel_target: &str) -> Result<String> {
    let path = resolve_target("ppt/slides/slide.xml", rel_target);
    let xml = bundle.text(&path)?;
    let doc = Document::parse(&xml)?;
    let name = doc
        .descendants()
        .find(|node| node.tag_name().name() == "cSld")
        .and_then(|node| attr_value(&node, "name").map(str::to_string))
        .unwrap_or_else(|| "Unknown".to_string());
    Ok(name)
}

pub fn extract_layout_info(bundle: &ZipBundle, path: &str) -> Result<LayoutInfo> {
    let xml = bundle.text(path)?;
    let doc = Document::parse(&xml)?;
    let name = doc
        .descendants()
        .find(|node| node.tag_name().name() == "cSld")
        .and_then(|node| attr_value(&node, "name").map(str::to_string))
        .unwrap_or_else(|| "Unknown".to_string());
    let placeholders = doc
        .descendants()
        .filter(|node| node.tag_name().name() == "ph")
        .map(|node| LayoutPlaceholder {
            idx: attr_value(&node, "idx").map(str::to_string),
            name: node
                .ancestors()
                .find(|ancestor| ancestor.tag_name().name() == "sp")
                .and_then(|shape| {
                    shape
                        .children()
                        .find(|child| child.tag_name().name() == "nvSpPr")
                })
                .and_then(|nv| {
                    nv.descendants()
                        .find(|child| child.tag_name().name() == "cNvPr")
                })
                .and_then(|nv| attr_value(&nv, "name").map(str::to_string))
                .unwrap_or_default(),
        })
        .collect::<Vec<_>>();
    Ok(LayoutInfo { name, placeholders })
}

pub fn extract_notes(bundle: &ZipBundle, rel_target: &str) -> Result<String> {
    let path = resolve_target("ppt/slides/slide.xml", rel_target);
    let xml = bundle.text(&path)?;
    let doc = Document::parse(&xml)?;
    Ok(collect_text(&doc.root_element()))
}

pub fn extract_slide_elements(
    bundle: &ZipBundle,
    slide_doc: &Document<'_>,
    slide_rels: &HashMap<String, String>,
    slide_index: usize,
    extract_images: bool,
    image_dir: Option<&Path>,
) -> Result<Vec<ElementInfo>> {
    let sp_tree = slide_doc
        .descendants()
        .find(|node| node.tag_name().name() == "spTree")
        .ok_or_else(|| anyhow!("slide missing spTree"))?;
    let mut elements = Vec::new();
    let mut element_index = 0;
    for child in sp_tree.children().filter(|node| node.is_element()) {
        let local = child.tag_name().name();
        if !matches!(local, "sp" | "pic" | "graphicFrame" | "grpSp") {
            continue;
        }
        element_index += 1;
        elements.push(extract_element(
            bundle,
            &child,
            slide_rels,
            slide_index,
            element_index,
            extract_images,
            image_dir,
        )?);
    }
    Ok(elements)
}

pub fn extract_element(
    bundle: &ZipBundle,
    node: &Node<'_, '_>,
    slide_rels: &HashMap<String, String>,
    slide_index: usize,
    shape_index: usize,
    extract_images: bool,
    image_dir: Option<&Path>,
) -> Result<ElementInfo> {
    let name = node
        .descendants()
        .find(|child| child.tag_name().name() == "cNvPr")
        .and_then(|child| attr_value(&child, "name").map(str::to_string))
        .unwrap_or_default();
    let mut element = ElementInfo {
        index: shape_index,
        name,
        element_type: match node.tag_name().name() {
            "sp" => "shape",
            "pic" => "image",
            "graphicFrame" => "graphicFrame",
            "grpSp" => "group",
            other => other,
        }
        .to_string(),
        position: extract_position(node),
        rotation: extract_rotation(node),
        text: extract_text_info(node),
        image: None,
        table: None,
        chart: None,
        children: None,
    };

    match node.tag_name().name() {
        "pic" => {
            let embed_id = node
                .descendants()
                .find(|child| child.tag_name().name() == "blip")
                .and_then(|child| rel_attr_value(&child, "embed").map(str::to_string));
            if let Some(embed_id) = embed_id {
                let info = extract_image_info(
                    bundle,
                    slide_rels,
                    &embed_id,
                    slide_index,
                    shape_index,
                    extract_images,
                    image_dir,
                )?;
                element.image = Some(info);
            }
        }
        "graphicFrame" => {
            if let Some(tbl) = node
                .descendants()
                .find(|child| child.tag_name().name() == "tbl")
            {
                element.element_type = "table".to_string();
                element.table = Some(extract_table_info(&tbl));
            } else if let Some(chart) = node
                .descendants()
                .find(|child| child.tag_name().name() == "chart")
            {
                element.element_type = "chart".to_string();
                let rel_id = rel_attr_value(&chart, "id").unwrap_or("chart");
                element.chart = Some(ChartInfo {
                    chart_type: rel_id.to_string(),
                    has_legend: None,
                });
            }
        }
        "grpSp" => {
            element.element_type = "group".to_string();
            let mut children = Vec::new();
            let mut child_index = 0;
            for child in node.children().filter(|child| child.is_element()) {
                if !matches!(
                    child.tag_name().name(),
                    "sp" | "pic" | "graphicFrame" | "grpSp"
                ) {
                    continue;
                }
                child_index += 1;
                children.push(extract_element(
                    bundle,
                    &child,
                    slide_rels,
                    slide_index,
                    child_index,
                    extract_images,
                    image_dir,
                )?);
            }
            element.children = Some(children);
        }
        _ => {}
    }
    Ok(element)
}

pub fn extract_image_info(
    bundle: &ZipBundle,
    slide_rels: &HashMap<String, String>,
    embed_id: &str,
    slide_index: usize,
    shape_index: usize,
    extract_images: bool,
    image_dir: Option<&Path>,
) -> Result<ImageInfo> {
    let target = slide_rels
        .get(embed_id)
        .ok_or_else(|| anyhow!("missing image relationship {}", embed_id))?;
    let media_path = resolve_target("ppt/slides/slide.xml", target);
    let bytes = bundle
        .read_bytes(&media_path)
        .with_context(|| format!("missing media {}", media_path))?;
    let image = image::load_from_memory(&bytes).ok();
    let content_type = media_path
        .rsplit('.')
        .next()
        .map(|ext| format!("image/{}", ext));
    let extracted_path = if extract_images {
        if let Some(dir) = image_dir {
            fs::create_dir_all(dir)?;
            let ext = media_path.rsplit('.').next().unwrap_or("bin");
            let path = dir.join(format!(
                "slide{}_shape{}.{}",
                slide_index + 1,
                shape_index,
                ext
            ));
            fs::write(&path, bytes)?;
            Some(path.display().to_string())
        } else {
            None
        }
    } else {
        None
    };
    Ok(ImageInfo {
        content_type,
        width: image.as_ref().map(DynamicImage::width),
        height: image.as_ref().map(DynamicImage::height),
        extracted_path,
    })
}

pub fn extract_table_info(node: &Node<'_, '_>) -> TableInfo {
    let rows = node
        .children()
        .filter(|child| child.is_element() && child.tag_name().name() == "tr")
        .map(|row| {
            row.children()
                .filter(|cell| cell.is_element() && cell.tag_name().name() == "tc")
                .map(|cell| collect_text(&cell))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    TableInfo {
        rows: rows.len(),
        cols,
        data: rows,
    }
}

pub fn extract_position(node: &Node<'_, '_>) -> Position {
    let xfrm = node
        .descendants()
        .find(|child| matches!(child.tag_name().name(), "xfrm" | "off" | "ext"));
    let (x, y, w, h) = if let Some(xfrm) = xfrm {
        if xfrm.tag_name().name() == "xfrm" {
            let off = xfrm
                .children()
                .find(|child| child.tag_name().name() == "off");
            let ext = xfrm
                .children()
                .find(|child| child.tag_name().name() == "ext");
            (
                off.and_then(|node| attr_f64(&node, "x"))
                    .unwrap_or_default()
                    / EMU_PER_INCH,
                off.and_then(|node| attr_f64(&node, "y"))
                    .unwrap_or_default()
                    / EMU_PER_INCH,
                ext.and_then(|node| attr_f64(&node, "cx"))
                    .unwrap_or_default()
                    / EMU_PER_INCH,
                ext.and_then(|node| attr_f64(&node, "cy"))
                    .unwrap_or_default()
                    / EMU_PER_INCH,
            )
        } else {
            (0.0, 0.0, 0.0, 0.0)
        }
    } else {
        (0.0, 0.0, 0.0, 0.0)
    };
    Position {
        x: round4(x),
        y: round4(y),
        w: round4(w),
        h: round4(h),
    }
}

pub fn extract_rotation(node: &Node<'_, '_>) -> Option<f64> {
    node.descendants()
        .find(|child| child.tag_name().name() == "xfrm")
        .and_then(|xfrm| attr_f64(&xfrm, "rot"))
        .map(|rot| rot / 60_000.0)
        .filter(|rot| *rot != 0.0)
}

pub fn extract_text_info(node: &Node<'_, '_>) -> Option<TextInfo> {
    let text_node = node
        .descendants()
        .find(|child| child.tag_name().name() == "txBody")?;
    let paragraphs = text_node
        .children()
        .filter(|child| child.is_element() && child.tag_name().name() == "p")
        .map(|paragraph| ParagraphInfo {
            text: collect_text(&paragraph),
        })
        .collect::<Vec<_>>();
    let full_text = paragraphs
        .iter()
        .map(|item| item.text.clone())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    Some(TextInfo {
        full_text,
        paragraphs,
    })
}

pub fn collect_text(node: &Node<'_, '_>) -> String {
    node.descendants()
        .filter(|child| child.is_element() && child.tag_name().name() == "t")
        .filter_map(|child| child.text())
        .collect::<Vec<_>>()
        .join("")
}

pub fn attr_value<'a>(node: &'a Node<'a, 'a>, key: &str) -> Option<&'a str> {
    node.attribute(key).or_else(|| {
        key.split_once(':')
            .and_then(|(_, local)| node.attribute(local))
    })
}

pub fn rel_attr_value<'a>(node: &'a Node<'a, 'a>, local: &str) -> Option<&'a str> {
    node.attribute((
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
        local,
    ))
    .or_else(|| node.attribute(local))
}

pub fn attr_f64(node: &Node<'_, '_>, key: &str) -> Option<f64> {
    attr_value(node, key).and_then(|value| value.parse::<f64>().ok())
}

pub fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}

pub fn element_overflows(element: &Value, slide_w: f64, slide_h: f64) -> bool {
    let position = element.get("position");
    let x = position
        .and_then(|pos| pos.get("x"))
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let y = position
        .and_then(|pos| pos.get("y"))
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let w = position
        .and_then(|pos| pos.get("w"))
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let h = position
        .and_then(|pos| pos.get("h"))
        .and_then(Value::as_f64)
        .unwrap_or_default();
    let over = x < -0.01 || y < -0.01 || x + w > slide_w + 0.01 || y + h > slide_h + 0.01;
    if over {
        return true;
    }
    element
        .get("children")
        .and_then(Value::as_array)
        .map(|children| {
            children
                .iter()
                .any(|child| element_overflows(child, slide_w, slide_h))
        })
        .unwrap_or(false)
}

pub fn has_text_bbox_overlap(elements: &[Value]) -> bool {
    let mut boxes = Vec::new();
    for element in elements {
        collect_text_boxes(element, &mut boxes);
    }
    for i in 0..boxes.len() {
        for j in (i + 1)..boxes.len() {
            if rects_overlap(&boxes[i], &boxes[j]) {
                return true;
            }
        }
    }
    false
}

pub fn has_dense_text_overlap_risk(elements: &[Value]) -> bool {
    let mut boxes = Vec::new();
    for element in elements {
        collect_text_boxes(element, &mut boxes);
    }
    if boxes.len() < 6 {
        return false;
    }
    let mut overlap_pairs = 0usize;
    for i in 0..boxes.len() {
        for j in (i + 1)..boxes.len() {
            if rects_overlap(&boxes[i], &boxes[j]) {
                overlap_pairs += 1;
            }
        }
    }
    let pair_count = boxes.len() * (boxes.len() - 1) / 2;
    let overlap_density = overlap_pairs as f64 / pair_count as f64;
    overlap_density >= 0.20
}

pub fn has_dense_text_density_risk(elements: &[Value]) -> bool {
    let stats = slide_text_stats(elements);
    // Conservative thresholds tuned to catch document-like slides:
    // - too many text boxes (card farm / label spam)
    // - too much total text even if boxes do not overlap
    stats.text_box_count >= 9
        || (stats.text_box_count >= 6 && stats.text_char_count >= 380)
        || stats.text_char_count >= 520
}

pub fn has_ai_copy_slop_risk(elements: &[Value]) -> bool {
    let mut hits = 0usize;
    for element in elements {
        hits += count_ai_slop_hits_in_element(element);
        if hits >= 2 {
            return true;
        }
    }
    false
}

pub fn count_ai_slop_hits_in_element(element: &Value) -> usize {
    let mut hits = 0usize;
    let text = element
        .pointer("/text/fullText")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if !text.is_empty() {
        hits += count_ai_slop_hits(text);
    }
    if let Some(children) = element.get("children").and_then(Value::as_array) {
        for child in children {
            hits += count_ai_slop_hits_in_element(child);
        }
    }
    hits
}

pub fn count_ai_slop_hits(text: &str) -> usize {
    // High-precision only: fail on unmistakably templated filler.
    const HARD_PHRASES: [&str; 60] = [
        "赋能",
        "闭环",
        "抓手",
        "落地",
        "顶层设计",
        "生态",
        "矩阵",
        "方法论",
        "全链路",
        "全方位",
        "一站式",
        "端到端",
        "体系化",
        "长效机制",
        "协同发力",
        "统筹推进",
        "高标准推进",
        "高质量发展",
        "对标行业领先",
        "行业领先",
        "价值最大化",
        "最优解",
        "精准触达",
        "精细化运营",
        "提效增能",
        "降本增效",
        "强强联合",
        "全面提升",
        "显著提升",
        "进一步提升",
        "持续优化",
        "打造",
        "助力",
        "提升效率",
        "增强能力",
        "赋能业务",
        "具有重要意义",
        "核心观点如下",
        "综上所述",
        "值得关注的是",
        "本页展示",
        "本页主要展示",
        "本页重点展示",
        "本页呈现",
        "本页介绍",
        "本页说明",
        "方案如下",
        "总体来说",
        "总体而言",
        "在此基础上",
        "需要指出的是",
        "需要说明的是",
        "阶段性成果",
        "里程碑意义",
        "多维度",
        "立体化",
        "全覆盖",
        "全面推进",
        "稳步推进",
        "有序推进",
    ];
    let mut hits = 0usize;
    for phrase in HARD_PHRASES {
        if text.contains(phrase) {
            hits += 1;
        }
    }
    hits + vague_promise_hits(text)
}

pub fn vague_promise_hits(text: &str) -> usize {
    if has_any_evidence_token(text) {
        return 0;
    }
    const PROMISE_VERBS: [&str; 11] = [
        "提升", "优化", "完善", "增强", "加强", "保障", "推进", "促进", "改善", "降低", "构建",
    ];
    let mut hits = 0usize;
    for verb in PROMISE_VERBS {
        if text.contains(verb) {
            hits += 1;
        }
    }
    if text.contains("打造") {
        hits += 1;
    }
    hits.min(2)
}

pub fn has_any_evidence_token(text: &str) -> bool {
    if text.chars().any(|ch| ch.is_ascii_digit()) {
        return true;
    }
    const TOKENS: [&str; 26] = [
        "%", "pp", "ms", "秒", "分钟", "小时", "天", "周", "月", "季度", "年", "截至", "本周", "下周",
        "本月", "下月", "Q1", "Q2", "Q3", "Q4", "样本", "n=", "N=", "口径", "对比", "基准",
    ];
    TOKENS.iter().any(|t| text.contains(t))
}

#[derive(Debug, Clone, Copy)]
pub struct SlideTextStats {
    text_box_count: usize,
    text_char_count: usize,
}

pub fn slide_text_stats(elements: &[Value]) -> SlideTextStats {
    let mut stats = SlideTextStats {
        text_box_count: 0,
        text_char_count: 0,
    };
    for element in elements {
        collect_text_stats(element, &mut stats);
    }
    stats
}

pub fn collect_text_stats(element: &Value, stats: &mut SlideTextStats) {
    let full_text = element
        .pointer("/text/fullText")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if !full_text.is_empty() {
        stats.text_box_count += 1;
        stats.text_char_count += full_text.chars().count();
    }
    if let Some(children) = element.get("children").and_then(Value::as_array) {
        for child in children {
            collect_text_stats(child, stats);
        }
    }
}

pub fn slide_has_rust_semantic_marker(slide: &Value) -> bool {
    slide
        .get("notes")
        .and_then(Value::as_str)
        .is_some_and(|n| n.contains("rust_semantic_layout:"))
}

pub fn semantic_layout_role_from_slide(slide: &Value) -> String {
    slide
        .get("notes")
        .and_then(Value::as_str)
        .and_then(|notes| {
            notes.lines().find_map(|line| {
                line.strip_prefix("rust_semantic_layout:")
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
            })
        })
        .unwrap_or_else(|| "__semantic_missing__".to_string())
}

pub fn deck_notes_have_semantic_markers(slides: &[Value]) -> bool {
    slides.iter().any(slide_has_rust_semantic_marker)
}

pub fn layout_rhythm_failing_slides(slides: &[Value]) -> Vec<usize> {
    let slide_count = slides.len();
    if slide_count < 6 {
        return Vec::new();
    }
    // Rust-generated decks share one OOXML slide layout (`Rust Blank`). Use speaker-note
    // `rust_semantic_layout:*` markers for rhythm checks; skip when absent (foreign decks).
    if !deck_notes_have_semantic_markers(slides) {
        return Vec::new();
    }

    let mut layouts: Vec<String> = Vec::with_capacity(slide_count);
    let mut counts: HashMap<String, usize> = HashMap::new();
    for slide in slides {
        let role = semantic_layout_role_from_slide(slide);
        *counts.entry(role.clone()).or_insert(0) += 1;
        layouts.push(role);
    }

    let mut failing: BTreeSet<usize> = BTreeSet::new();

    // Never allow 3 consecutive slides with the same layout name.
    let mut run_start = 0usize;
    while run_start < layouts.len() {
        let mut run_end = run_start + 1;
        while run_end < layouts.len() && layouts[run_end] == layouts[run_start] {
            run_end += 1;
        }
        let run_len = run_end - run_start;
        if run_len >= 3 && !layouts[run_start].is_empty() {
            // Mark the 3rd and onward as failing to encourage earlier role variation.
            for idx in (run_start + 2)..run_end {
                failing.insert(idx + 1); // 1-based slide indexing
            }
        }
        run_start = run_end;
    }

    // Whole-deck isomorphic layout repetition: if one semantic role dominates too much.
    let max_layout_share = counts
        .values()
        .copied()
        .max()
        .unwrap_or(0) as f64
        / slide_count as f64;
    if max_layout_share >= 0.80 {
        for i in 0..slide_count {
            failing.insert(i + 1);
        }
    }

    failing.into_iter().collect()
}

pub fn has_decorative_title_underline_risk(elements: &[Value], slide_w: f64, slide_h: f64) -> bool {
    let title_box = match find_title_candidate(elements, slide_h) {
        Some(rect) => rect,
        None => return false,
    };

    let window_top = title_box.3;
    let window_bottom = (title_box.3 + 0.35).min(slide_h);
    let min_w = slide_w * 0.35;
    let max_h = 0.12;

    for element in elements {
        let full_text = element
            .pointer("/text/fullText")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        if !full_text.is_empty() {
            continue;
        }
        let Some((x1, y1, x2, y2)) = element_rect(element) else {
            continue;
        };
        let w = x2 - x1;
        let h = y2 - y1;
        if w < min_w || h <= 0.0 || h > max_h {
            continue;
        }
        let y_mid = (y1 + y2) / 2.0;
        if y_mid < window_top || y_mid > window_bottom {
            continue;
        }
        if (x1 - title_box.0).abs() > 0.6 {
            continue;
        }
        return true;
    }
    false
}

pub fn find_title_candidate(elements: &[Value], slide_h: f64) -> Option<(f64, f64, f64, f64)> {
    let top_zone_max_y = slide_h * 0.28;
    let mut best: Option<(f64, f64, f64, f64, f64)> = None;
    for element in elements {
        let full_text = element
            .pointer("/text/fullText")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        if full_text.is_empty() {
            continue;
        }
        let Some((x1, y1, x2, y2)) = element_rect(element) else {
            continue;
        };
        if y1 > top_zone_max_y {
            continue;
        }
        let w = x2 - x1;
        let h = y2 - y1;
        if w <= 0.0 || h <= 0.0 {
            continue;
        }
        let area = w * h;
        match best {
            Some((_, _, _, _, best_area)) if area <= best_area => {}
            _ => best = Some((x1, y1, x2, y2, area)),
        }
    }
    best.map(|(x1, y1, x2, y2, _)| (x1, y1, x2, y2))
}

pub fn element_rect(element: &Value) -> Option<(f64, f64, f64, f64)> {
    let position = element.get("position")?;
    let x = position.get("x").and_then(Value::as_f64)?;
    let y = position.get("y").and_then(Value::as_f64)?;
    let w = position.get("w").and_then(Value::as_f64)?;
    let h = position.get("h").and_then(Value::as_f64)?;
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    Some((x, y, x + w, y + h))
}

pub fn collect_text_boxes(element: &Value, boxes: &mut Vec<(f64, f64, f64, f64)>) {
    let full_text = element
        .pointer("/text/fullText")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if !full_text.is_empty() {
        let position = element.get("position");
        let x = position
            .and_then(|pos| pos.get("x"))
            .and_then(Value::as_f64)
            .unwrap_or_default();
        let y = position
            .and_then(|pos| pos.get("y"))
            .and_then(Value::as_f64)
            .unwrap_or_default();
        let w = position
            .and_then(|pos| pos.get("w"))
            .and_then(Value::as_f64)
            .unwrap_or_default();
        let h = position
            .and_then(|pos| pos.get("h"))
            .and_then(Value::as_f64)
            .unwrap_or_default();
        if w > 0.0 && h > 0.0 {
            boxes.push((x, y, x + w, y + h));
        }
    }
    if let Some(children) = element.get("children").and_then(Value::as_array) {
        for child in children {
            collect_text_boxes(child, boxes);
        }
    }
}

pub fn rects_overlap(a: &(f64, f64, f64, f64), b: &(f64, f64, f64, f64)) -> bool {
    let tol = 0.01;
    a.0 < b.2 - tol && a.2 > b.0 + tol && a.1 < b.3 - tol && a.3 > b.1 + tol
}

pub fn extract_requested_fonts_by_slide(
    bundle: &ZipBundle,
) -> Result<BTreeMap<usize, BTreeSet<String>>> {
    let defaults = extract_theme_fonts(bundle)?;
    let mut by_slide = BTreeMap::new();
    let mut slide_names = bundle
        .names()
        .filter(|name| name.starts_with("ppt/slides/slide") && name.ends_with(".xml"))
        .cloned()
        .collect::<Vec<_>>();
    slide_names.sort();
    for (index, slide_name) in slide_names.iter().enumerate() {
        let xml = bundle.text(slide_name)?;
        let doc = Document::parse(&xml)?;
        let mut fonts = BTreeSet::new();
        for node in doc.descendants() {
            match node.tag_name().name() {
                "latin" | "ea" | "cs" | "sym" | "font" => {
                    if let Some(face) = attr_value(&node, "typeface") {
                        if !face.trim().is_empty() && face != "+mn-lt" && face != "+mj-lt" {
                            fonts.insert(face.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
        if fonts.is_empty() {
            fonts.extend(defaults.iter().cloned());
        }
        by_slide.insert(index + 1, fonts);
    }
    Ok(by_slide)
}

pub fn extract_theme_fonts(bundle: &ZipBundle) -> Result<BTreeSet<String>> {
    let theme_name = bundle
        .names()
        .find(|name| name.starts_with("ppt/theme/theme") && name.ends_with(".xml"))
        .cloned()
        .ok_or_else(|| anyhow!("missing theme xml"))?;
    let xml = bundle.text(&theme_name)?;
    let doc = Document::parse(&xml)?;
    let mut fonts = BTreeSet::new();
    for node in doc
        .descendants()
        .filter(|node| matches!(node.tag_name().name(), "latin" | "ea" | "cs"))
    {
        if let Some(face) = attr_value(&node, "typeface") {
            if !face.trim().is_empty() {
                fonts.insert(face.to_string());
            }
        }
    }
    Ok(fonts)
}

pub fn normalize_font_family_name(name: &str) -> String {
    fn re_paren() -> &'static Regex {
        static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
        RE.get_or_init(|| Regex::new(r"\([^)]*\)").unwrap())
    }
    fn re_sep() -> &'static Regex {
        static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
        RE.get_or_init(|| Regex::new(r#"[\s\-\_\.,/\'\"]+"#).unwrap())
    }
    let lower = name.to_lowercase();
    let no_paren = re_paren().replace_all(&lower, " ");
    let cleaned = re_sep().replace_all(&no_paren, " ");
    cleaned.trim().to_string()
}

pub fn build_font_synonym_map() -> Result<HashMap<String, BTreeSet<String>>> {
    let output = run_command_capture(
        Command::new("fc-list")
            .arg("--format")
            .arg("%{family}\t%{fullname}\t%{postscriptname}\n"),
    )
    .context("fc-list failed")?;
    let mut syn = HashMap::<String, BTreeSet<String>>::new();
    for line in output.lines() {
        let parts = line.split('\t').collect::<Vec<_>>();
        if parts.len() != 3 {
            continue;
        }
        let mut names = BTreeSet::new();
        for field in parts {
            for item in field.split(',') {
                let normalized = normalize_font_family_name(item);
                if !normalized.is_empty() {
                    names.insert(normalized.clone());
                    names.insert(normalized.replace(' ', ""));
                }
            }
        }
        for name in names.clone() {
            syn.entry(name).or_default().extend(names.clone());
        }
    }
    Ok(syn)
}

pub fn expand_font_family_aliases(
    synonyms: &HashMap<String, BTreeSet<String>>,
    family: &str,
) -> BTreeSet<String> {
    let mut acceptable = BTreeSet::from([family.to_string(), family.replace(' ', "")]);
    if let Some(items) = synonyms.get(family) {
        acceptable.extend(items.iter().cloned());
    }
    let compact = family.replace(' ', "");
    if let Some(items) = synonyms.get(&compact) {
        acceptable.extend(items.iter().cloned());
    }
    acceptable
}

pub fn extract_resolved_fonts_from_odp(input: &Path) -> Result<BTreeSet<String>> {
    let profile = TempDir::new()?;
    let convert_dir = TempDir::new()?;
    let profile_flag = format!("file://{}", profile.path().display());
    let mut convert = Command::new("soffice");
    convert
        .arg(format!("-env:UserInstallation={}", profile_flag))
        .arg("--invisible")
        .arg("--headless")
        .arg("--norestore")
        .arg("--convert-to")
        .arg("odp")
        .arg("--outdir")
        .arg(convert_dir.path())
        .arg(input)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    run_command_timeout(&mut convert, SOFFICE_PROBE_TIMEOUT)?;
    let stem = input.file_stem().and_then(OsStr::to_str).unwrap_or("deck");
    let odp_path = convert_dir.path().join(format!("{}.odp", stem));
    let bundle = ZipBundle::from_path(&odp_path)?;
    let mut fonts = BTreeSet::new();
    let font_re = Regex::new(r#"font-family[^=]*=\"([^\"]+)\""#)?;
    for target in ["content.xml", "styles.xml"] {
        let text = match bundle.text(target) {
            Ok(text) => text,
            Err(_) => continue,
        };
        for caps in font_re.captures_iter(&text) {
            for value in caps[1].split(',') {
                let normalized = normalize_font_family_name(value.trim_matches('"').trim());
                if !normalized.is_empty() {
                    fonts.insert(normalized);
                }
            }
        }
    }
    Ok(fonts)
}

pub fn sanitize_presentation_xml(xml: &str) -> Result<String> {
    let notes_master_re =
        Regex::new(r#"(?s)<p:notesMasterIdLst(?:\s*/>|>.*?</p:notesMasterIdLst>)"#)?;
    let sld_master_re = Regex::new(r#"(?s)<p:sldMasterIdLst(?:\s*/>|>.*?</p:sldMasterIdLst>)"#)?;

    let notes_master = match notes_master_re.find(xml) {
        Some(value) => value.as_str().to_string(),
        None => return Ok(xml.to_string()),
    };
    let without_notes_master = notes_master_re.replace(xml, "").to_string();
    if let Some(sld_master) = sld_master_re.find(&without_notes_master) {
        let mut rebuilt = String::with_capacity(without_notes_master.len() + notes_master.len());
        rebuilt.push_str(&without_notes_master[..sld_master.end()]);
        rebuilt.push_str(&notes_master);
        rebuilt.push_str(&without_notes_master[sld_master.end()..]);
        return Ok(rebuilt);
    }
    Ok(without_notes_master)
}

pub fn join_display_list(value: Option<&Vec<Value>>) -> String {
    value
        .into_iter()
        .flat_map(|items| items.iter())
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

