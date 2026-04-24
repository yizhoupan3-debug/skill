use anyhow::{anyhow, bail, Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use clap::{Args, Parser, Subcommand};
use image::{DynamicImage, ImageFormat};
use reqwest::blocking::Client;
use reqwest::StatusCode;
use serde_json::{json, Map, Value};
use std::collections::VecDeque;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const DEFAULT_MODEL: &str = "gpt-5.4";
const DEFAULT_RESPONSES_URL: &str = "http://127.0.0.1:8318/v1/responses";
const DEFAULT_SIZE: &str = "1024x1024";
const DEFAULT_QUALITY: &str = "auto";
const DEFAULT_OUTPUT_FORMAT: &str = "png";
const DEFAULT_OUTPUT_PATH: &str = "output/imagegen/output.png";
const DEFAULT_DOWNSCALE_SUFFIX: &str = "-web";
const DEFAULT_CONCURRENCY: usize = 5;
const DEFAULT_TIMEOUT_SECONDS: u64 = 300;
const MAX_IMAGE_BYTES: u64 = 50 * 1024 * 1024;
const MAX_BATCH_JOBS: usize = 500;

#[derive(Parser)]
#[command(name = "image_gen_rs")]
#[command(about = "Generate or edit images via VibeProxy Local /v1/responses")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Generate(GenerateArgs),
    Edit(EditArgs),
    GenerateBatch(BatchArgs),
}

#[derive(Args, Clone)]
struct SharedArgs {
    #[arg(long, default_value = DEFAULT_MODEL)]
    model: String,
    #[arg(long)]
    prompt: Option<String>,
    #[arg(long)]
    prompt_file: Option<PathBuf>,
    #[arg(long, default_value_t = 1)]
    n: usize,
    #[arg(long, default_value = DEFAULT_SIZE)]
    size: String,
    #[arg(long, default_value = DEFAULT_QUALITY)]
    quality: String,
    #[arg(long)]
    background: Option<String>,
    #[arg(long)]
    output_format: Option<String>,
    #[arg(long)]
    output_compression: Option<u8>,
    #[arg(long)]
    moderation: Option<String>,
    #[arg(long, default_value = DEFAULT_OUTPUT_PATH)]
    out: PathBuf,
    #[arg(long)]
    out_dir: Option<PathBuf>,
    #[arg(long)]
    force: bool,
    #[arg(long)]
    dry_run: bool,
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    augment: bool,
    #[arg(long)]
    use_case: Option<String>,
    #[arg(long)]
    scene: Option<String>,
    #[arg(long)]
    subject: Option<String>,
    #[arg(long)]
    style: Option<String>,
    #[arg(long)]
    composition: Option<String>,
    #[arg(long)]
    lighting: Option<String>,
    #[arg(long)]
    palette: Option<String>,
    #[arg(long)]
    materials: Option<String>,
    #[arg(long)]
    text: Option<String>,
    #[arg(long)]
    constraints: Option<String>,
    #[arg(long)]
    negative: Option<String>,
    #[arg(long)]
    downscale_max_dim: Option<u32>,
    #[arg(long, default_value = DEFAULT_DOWNSCALE_SUFFIX)]
    downscale_suffix: String,
    #[arg(long, default_value_t = 3)]
    max_attempts: usize,
}

#[derive(Args, Clone)]
struct GenerateArgs {
    #[command(flatten)]
    shared: SharedArgs,
}

#[derive(Args, Clone)]
struct EditArgs {
    #[command(flatten)]
    shared: SharedArgs,
    #[arg(long, required = true)]
    image: Vec<PathBuf>,
    #[arg(long)]
    mask: Option<PathBuf>,
    #[arg(long)]
    input_fidelity: Option<String>,
}

#[derive(Args, Clone)]
struct BatchArgs {
    #[command(flatten)]
    shared: SharedArgs,
    #[arg(long)]
    input: PathBuf,
    #[arg(long, default_value_t = DEFAULT_CONCURRENCY)]
    concurrency: usize,
    #[arg(long)]
    fail_fast: bool,
}

#[derive(Clone)]
struct BatchJob {
    prompt: String,
    raw: Map<String, Value>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Generate(args) => run_generate(args),
        Command::Edit(args) => run_edit(args),
        Command::GenerateBatch(args) => run_batch(args),
    }
}

fn run_generate(args: GenerateArgs) -> Result<()> {
    validate_shared(&args.shared)?;
    let prompt = augment_prompt(
        &args.shared,
        read_prompt(&args.shared)?,
        &fields_from_shared(&args.shared),
    );
    let output_format = normalize_output_format(args.shared.output_format.as_deref())?;
    validate_transparency(args.shared.background.as_deref(), &output_format)?;
    let outputs = build_output_paths(
        &args.shared.out,
        &output_format,
        args.shared.n,
        args.shared.out_dir.as_deref(),
    )?;

    if args.shared.dry_run {
        let mut payload = build_generate_payload(&args.shared, &prompt)?;
        payload["endpoint"] = json!(responses_url());
        payload["outputs"] = json!(outputs.iter().map(path_string).collect::<Vec<_>>());
        print_json(&payload)?;
        return Ok(());
    }

    generate_many(&args.shared, &prompt, &outputs, "[generate]")
}

fn run_edit(args: EditArgs) -> Result<()> {
    validate_shared(&args.shared)?;
    validate_edit(&args)?;
    let prompt = augment_prompt(
        &args.shared,
        read_prompt(&args.shared)?,
        &fields_from_shared(&args.shared),
    );
    let output_format = normalize_output_format(args.shared.output_format.as_deref())?;
    validate_transparency(args.shared.background.as_deref(), &output_format)?;
    let outputs = build_output_paths(
        &args.shared.out,
        &output_format,
        args.shared.n,
        args.shared.out_dir.as_deref(),
    )?;

    if args.shared.dry_run {
        let mut payload = build_edit_payload(
            &args.shared,
            &prompt,
            &args.image,
            args.input_fidelity.as_deref(),
            true,
        )?;
        payload["endpoint"] = json!(responses_url());
        payload["outputs"] = json!(outputs.iter().map(path_string).collect::<Vec<_>>());
        print_json(&payload)?;
        return Ok(());
    }

    edit_many(&args, &prompt, &outputs, "[edit]")
}

fn run_batch(args: BatchArgs) -> Result<()> {
    validate_shared(&args.shared)?;
    let out_dir = args
        .shared
        .out_dir
        .as_deref()
        .ok_or_else(|| anyhow!("Missing output directory. Use --out-dir"))?;
    if args.concurrency == 0 || args.concurrency > 25 {
        bail!("--concurrency must be between 1 and 25");
    }
    let jobs = read_jobs_jsonl(&args.input)?;
    let base_fields = fields_from_shared(&args.shared);

    if args.shared.dry_run {
        for (idx, job) in jobs.iter().enumerate() {
            let job_args = shared_for_job(&args.shared, job)?;
            let fields = merged_fields(&base_fields, job);
            let prompt = augment_prompt(&job_args, job.prompt.clone(), &fields);
            let output_format = normalize_output_format(job_args.output_format.as_deref())?;
            validate_transparency(job_args.background.as_deref(), &output_format)?;
            let outputs = job_output_paths(
                out_dir,
                &output_format,
                idx + 1,
                &job.prompt,
                job_args.n,
                job.raw.get("out").and_then(Value::as_str),
            )?;
            let mut payload = build_generate_payload(&job_args, &prompt)?;
            payload["endpoint"] = json!(responses_url());
            payload["job"] = json!(idx + 1);
            payload["outputs"] = json!(outputs.iter().map(path_string).collect::<Vec<_>>());
            print_json(&payload)?;
        }
        return Ok(());
    }

    let queue = Arc::new(Mutex::new(
        jobs.into_iter()
            .enumerate()
            .map(|(idx, job)| (idx + 1, job))
            .collect::<VecDeque<_>>(),
    ));
    let any_failed = Arc::new(AtomicBool::new(false));
    let total = queue.lock().expect("queue lock").len();
    let workers = usize::min(args.concurrency, total.max(1));
    let mut handles = Vec::new();

    for _ in 0..workers {
        let queue = Arc::clone(&queue);
        let any_failed = Arc::clone(&any_failed);
        let args = args.clone();
        let base_fields = base_fields.clone();
        handles.push(thread::spawn(move || -> Result<()> {
            loop {
                let next = queue.lock().expect("queue lock").pop_front();
                let Some((idx, job)) = next else {
                    return Ok(());
                };
                let label = format!("[job {idx}/{total}]");
                let result = (|| -> Result<()> {
                    let job_args = shared_for_job(&args.shared, &job)?;
                    let fields = merged_fields(&base_fields, &job);
                    let prompt = augment_prompt(&job_args, job.prompt.clone(), &fields);
                    let output_format = normalize_output_format(job_args.output_format.as_deref())?;
                    validate_transparency(job_args.background.as_deref(), &output_format)?;
                    let out_dir = args
                        .shared
                        .out_dir
                        .as_deref()
                        .ok_or_else(|| anyhow!("Missing output directory. Use --out-dir"))?;
                    let outputs = job_output_paths(
                        out_dir,
                        &output_format,
                        idx,
                        &job.prompt,
                        job_args.n,
                        job.raw.get("out").and_then(Value::as_str),
                    )?;
                    generate_many(&job_args, &prompt, &outputs, &label)
                })();

                if let Err(err) = result {
                    eprintln!("{label} failed: {err:#}");
                    any_failed.store(true, Ordering::SeqCst);
                    if args.fail_fast {
                        bail!("{label} failed: {err:#}");
                    }
                }
            }
        }));
    }

    for handle in handles {
        handle
            .join()
            .map_err(|_| anyhow!("batch worker panicked"))??;
    }
    if any_failed.load(Ordering::SeqCst) {
        bail!("one or more batch jobs failed");
    }
    Ok(())
}

fn validate_shared(args: &SharedArgs) -> Result<()> {
    if args.n == 0 || args.n > 10 {
        bail!("--n must be between 1 and 10");
    }
    if args.max_attempts == 0 || args.max_attempts > 10 {
        bail!("--max-attempts must be between 1 and 10");
    }
    if !matches!(
        args.size.as_str(),
        "1024x1024" | "1536x1024" | "1024x1536" | "auto"
    ) {
        bail!("size must be one of 1024x1024, 1536x1024, 1024x1536, or auto");
    }
    if !matches!(args.quality.as_str(), "low" | "medium" | "high" | "auto") {
        bail!("quality must be one of low, medium, high, or auto");
    }
    if let Some(background) = args.background.as_deref() {
        if !matches!(background, "transparent" | "opaque" | "auto") {
            bail!("background must be one of transparent, opaque, or auto");
        }
    }
    if let Some(compression) = args.output_compression {
        if compression > 100 {
            bail!("output-compression must be between 0 and 100");
        }
    }
    Ok(())
}

fn validate_edit(args: &EditArgs) -> Result<()> {
    if args.mask.is_some() {
        bail!("--mask is not supported on the VibeProxy /v1/responses path yet");
    }
    if let Some(input_fidelity) = args.input_fidelity.as_deref() {
        if !matches!(input_fidelity, "low" | "high") {
            bail!("input-fidelity must be one of low or high");
        }
    }
    for path in &args.image {
        let meta = fs::metadata(path)
            .with_context(|| format!("image file not found: {}", path.display()))?;
        if meta.len() > MAX_IMAGE_BYTES {
            eprintln!("Warning: image exceeds 50MB limit: {}", path.display());
        }
    }
    Ok(())
}

fn validate_transparency(background: Option<&str>, output_format: &str) -> Result<()> {
    if background == Some("transparent") && !matches!(output_format, "png" | "webp") {
        bail!("transparent background requires output-format png or webp");
    }
    Ok(())
}

fn read_prompt(args: &SharedArgs) -> Result<String> {
    match (&args.prompt, &args.prompt_file) {
        (Some(_), Some(_)) => bail!("Use --prompt or --prompt-file, not both"),
        (Some(prompt), None) => Ok(prompt.trim().to_string()),
        (None, Some(path)) => Ok(fs::read_to_string(path)
            .with_context(|| format!("prompt file not found: {}", path.display()))?
            .trim()
            .to_string()),
        (None, None) => bail!("Missing prompt. Use --prompt or --prompt-file"),
    }
}

fn fields_from_shared(args: &SharedArgs) -> Map<String, Value> {
    let mut fields = Map::new();
    insert_opt(&mut fields, "use_case", args.use_case.as_deref());
    insert_opt(&mut fields, "scene", args.scene.as_deref());
    insert_opt(&mut fields, "subject", args.subject.as_deref());
    insert_opt(&mut fields, "style", args.style.as_deref());
    insert_opt(&mut fields, "composition", args.composition.as_deref());
    insert_opt(&mut fields, "lighting", args.lighting.as_deref());
    insert_opt(&mut fields, "palette", args.palette.as_deref());
    insert_opt(&mut fields, "materials", args.materials.as_deref());
    insert_opt(&mut fields, "text", args.text.as_deref());
    insert_opt(&mut fields, "constraints", args.constraints.as_deref());
    insert_opt(&mut fields, "negative", args.negative.as_deref());
    fields
}

fn insert_opt(fields: &mut Map<String, Value>, key: &str, value: Option<&str>) {
    if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
        fields.insert(key.to_string(), json!(value));
    }
}

fn augment_prompt(args: &SharedArgs, prompt: String, fields: &Map<String, Value>) -> String {
    if !args.augment {
        return prompt;
    }
    let mut sections = Vec::new();
    if let Some(value) = fields.get("use_case").and_then(Value::as_str) {
        sections.push(format!("Use case: {value}"));
    }
    sections.push(format!("Primary request: {prompt}"));
    for (key, label) in [
        ("scene", "Scene/backdrop"),
        ("subject", "Subject"),
        ("style", "Style/medium"),
        ("composition", "Composition/framing"),
        ("lighting", "Lighting/mood"),
        ("palette", "Color palette"),
        ("materials", "Materials/textures"),
    ] {
        if let Some(value) = fields.get(key).and_then(Value::as_str) {
            sections.push(format!("{label}: {value}"));
        }
    }
    if let Some(value) = fields.get("text").and_then(Value::as_str) {
        sections.push(format!("Text (verbatim): \"{value}\""));
    }
    if let Some(value) = fields.get("constraints").and_then(Value::as_str) {
        sections.push(format!("Constraints: {value}"));
    }
    if let Some(value) = fields.get("negative").and_then(Value::as_str) {
        sections.push(format!("Avoid: {value}"));
    }
    sections.join("\n")
}

fn normalize_output_format(format: Option<&str>) -> Result<String> {
    let format = format.unwrap_or(DEFAULT_OUTPUT_FORMAT).to_ascii_lowercase();
    match format.as_str() {
        "png" | "jpeg" | "webp" => Ok(format),
        "jpg" => Ok("jpeg".to_string()),
        _ => bail!("output-format must be png, jpeg, jpg, or webp"),
    }
}

fn build_output_paths(
    out: &Path,
    output_format: &str,
    count: usize,
    out_dir: Option<&Path>,
) -> Result<Vec<PathBuf>> {
    let ext = format!(".{output_format}");
    if let Some(out_dir) = out_dir {
        return Ok((1..=count)
            .map(|idx| out_dir.join(format!("image_{idx}{ext}")))
            .collect());
    }
    let mut out_path = out.to_path_buf();
    if out_path.exists() && out_path.is_dir() {
        return Ok((1..=count)
            .map(|idx| out_path.join(format!("image_{idx}{ext}")))
            .collect());
    }
    if out_path.extension().is_none() {
        out_path.set_extension(output_format);
    } else if out_path.extension().and_then(|value| value.to_str()) != Some(output_format) {
        eprintln!(
            "Warning: output extension does not match output-format {output_format}: {}",
            out_path.display()
        );
    }
    if count == 1 {
        return Ok(vec![out_path]);
    }
    let stem = out_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("image")
        .to_string();
    let suffix = out_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{value}"))
        .unwrap_or_else(|| ext.clone());
    Ok((1..=count)
        .map(|idx| out_path.with_file_name(format!("{stem}-{idx}{suffix}")))
        .collect())
}

fn build_tool(
    args: &SharedArgs,
    action: Option<&str>,
    input_fidelity: Option<&str>,
) -> Result<Value> {
    let output_format = normalize_output_format(args.output_format.as_deref())?;
    let mut tool = Map::new();
    tool.insert("type".to_string(), json!("image_generation"));
    tool.insert("size".to_string(), json!(args.size));
    tool.insert("quality".to_string(), json!(args.quality));
    tool.insert("output_format".to_string(), json!(output_format));
    if let Some(background) = &args.background {
        tool.insert("background".to_string(), json!(background));
    }
    if let Some(compression) = args.output_compression {
        tool.insert("output_compression".to_string(), json!(compression));
    }
    if let Some(moderation) = &args.moderation {
        tool.insert("moderation".to_string(), json!(moderation));
    }
    if let Some(action) = action {
        tool.insert("action".to_string(), json!(action));
    }
    if let Some(input_fidelity) = input_fidelity {
        tool.insert("input_fidelity".to_string(), json!(input_fidelity));
    }
    Ok(Value::Object(tool))
}

fn build_generate_payload(args: &SharedArgs, prompt: &str) -> Result<Value> {
    Ok(json!({
        "model": args.model,
        "input": prompt,
        "tools": [build_tool(args, None, None)?],
    }))
}

fn build_edit_payload(
    args: &SharedArgs,
    prompt: &str,
    image_paths: &[PathBuf],
    input_fidelity: Option<&str>,
    preview: bool,
) -> Result<Value> {
    let mut content = vec![json!({"type": "input_text", "text": prompt})];
    for path in image_paths {
        let image_url = if preview {
            format!(
                "data:{};base64,<omitted:{}>",
                guess_mime_type(path),
                path.display()
            )
        } else {
            image_to_data_url(path)?
        };
        content.push(json!({"type": "input_image", "image_url": image_url}));
    }
    Ok(json!({
        "model": args.model,
        "input": [{"role": "user", "content": content}],
        "tools": [build_tool(args, Some("edit"), input_fidelity)?],
    }))
}

fn generate_many(
    args: &SharedArgs,
    prompt: &str,
    outputs: &[PathBuf],
    job_label: &str,
) -> Result<()> {
    let output_format = normalize_output_format(args.output_format.as_deref())?;
    for (idx, out_path) in outputs.iter().enumerate() {
        let label = if outputs.len() > 1 {
            format!("{job_label} variant {}/{}", idx + 1, outputs.len())
        } else {
            job_label.to_string()
        };
        eprintln!("{label} -> POST {}", responses_url());
        let started = Instant::now();
        let response = request_with_retries(
            &build_generate_payload(args, prompt)?,
            args.max_attempts,
            &label,
        )?;
        eprintln!(
            "{label} completed in {:.1}s",
            started.elapsed().as_secs_f64()
        );
        let images = extract_generated_images(&response)?;
        decode_write_and_downscale(
            &images[..usize::min(1, images.len())],
            std::slice::from_ref(out_path),
            args.force,
            args.downscale_max_dim,
            &args.downscale_suffix,
            &output_format,
        )?;
    }
    Ok(())
}

fn edit_many(args: &EditArgs, prompt: &str, outputs: &[PathBuf], job_label: &str) -> Result<()> {
    let output_format = normalize_output_format(args.shared.output_format.as_deref())?;
    for (idx, out_path) in outputs.iter().enumerate() {
        let label = if outputs.len() > 1 {
            format!("{job_label} variant {}/{}", idx + 1, outputs.len())
        } else {
            job_label.to_string()
        };
        eprintln!(
            "{label} -> POST {} (edit with {} image(s))",
            responses_url(),
            args.image.len()
        );
        let started = Instant::now();
        let response = request_with_retries(
            &build_edit_payload(
                &args.shared,
                prompt,
                &args.image,
                args.input_fidelity.as_deref(),
                false,
            )?,
            args.shared.max_attempts,
            &label,
        )?;
        eprintln!(
            "{label} completed in {:.1}s",
            started.elapsed().as_secs_f64()
        );
        let images = extract_generated_images(&response)?;
        decode_write_and_downscale(
            &images[..usize::min(1, images.len())],
            std::slice::from_ref(out_path),
            args.shared.force,
            args.shared.downscale_max_dim,
            &args.shared.downscale_suffix,
            &output_format,
        )?;
    }
    Ok(())
}

fn responses_url() -> String {
    std::env::var("VIBEPROXY_RESPONSES_URL").unwrap_or_else(|_| DEFAULT_RESPONSES_URL.to_string())
}

fn request_with_retries(payload: &Value, attempts: usize, label: &str) -> Result<Value> {
    let mut last_error = None;
    for attempt in 1..=attempts {
        match post_responses_request(payload) {
            Ok(value) => return Ok(value),
            Err(err) => {
                let message = err.to_string();
                let transient = is_transient_error(&message);
                last_error = Some(err);
                if !transient || attempt == attempts {
                    break;
                }
                let sleep = Duration::from_secs(u64::min(60, 2_u64.pow(attempt as u32)));
                eprintln!(
                    "{label} attempt {attempt}/{attempts} failed ({message}); retrying in {:.1}s",
                    sleep.as_secs_f64()
                );
                thread::sleep(sleep);
            }
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow!("unknown request failure")))
}

fn post_responses_request(payload: &Value) -> Result<Value> {
    let client = Client::builder()
        .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECONDS))
        .build()?;
    let mut request = client
        .post(responses_url())
        .header("Accept", "application/json")
        .json(payload);
    if let Some(token) = std::env::var("VIBEPROXY_BEARER_TOKEN")
        .ok()
        .or_else(|| std::env::var("VIBEPROXY_API_KEY").ok())
    {
        request = request.bearer_auth(token);
    }
    let response = request
        .send()
        .context("failed to reach VibeProxy Responses endpoint")?;
    let status = response.status();
    let body = response.text().unwrap_or_default();
    if !status.is_success() {
        bail!(
            "HTTP {} from {}: {}",
            status.as_u16(),
            responses_url(),
            body
        );
    }
    serde_json::from_str(&body).context("invalid JSON returned by VibeProxy Responses endpoint")
}

fn is_transient_error(message: &str) -> bool {
    [
        "HTTP 408", "HTTP 409", "HTTP 429", "HTTP 500", "HTTP 502", "HTTP 503", "HTTP 504",
    ]
    .iter()
    .any(|marker| message.contains(marker))
        || message.contains("timed out")
        || message.contains("timeout")
        || message.contains("connection reset")
        || message.contains(&StatusCode::REQUEST_TIMEOUT.as_u16().to_string())
}

fn extract_generated_images(response: &Value) -> Result<Vec<String>> {
    let output = response
        .get("output")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("Responses payload missing `output` list"))?;
    let mut images = Vec::new();
    let mut output_types = Vec::new();
    for item in output {
        let Some(item) = item.as_object() else {
            continue;
        };
        if let Some(kind) = item.get("type").and_then(Value::as_str) {
            output_types.push(kind.to_string());
        }
        if item.get("type").and_then(Value::as_str) != Some("image_generation_call") {
            continue;
        }
        match item.get("result") {
            Some(Value::String(value)) if !value.is_empty() => images.push(value.clone()),
            Some(Value::Array(values)) => {
                images.extend(
                    values
                        .iter()
                        .filter_map(Value::as_str)
                        .map(ToOwned::to_owned),
                );
            }
            _ => {}
        }
    }
    if images.is_empty() {
        bail!(
            "Responses call completed without an `image_generation_call` result. Observed output types: {:?}",
            output_types
        );
    }
    Ok(images)
}

fn decode_write_and_downscale(
    images: &[String],
    outputs: &[PathBuf],
    force: bool,
    downscale_max_dim: Option<u32>,
    downscale_suffix: &str,
    output_format: &str,
) -> Result<()> {
    for (image_b64, out_path) in images.iter().zip(outputs.iter()) {
        if out_path.exists() && !force {
            bail!(
                "Output already exists: {} (use --force to overwrite)",
                out_path.display()
            );
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw = decode_image_result(image_b64)?;
        fs::write(out_path, &raw)?;
        println!("Wrote {}", out_path.display());

        if let Some(max_dim) = downscale_max_dim {
            let derived = derive_downscale_path(out_path, downscale_suffix);
            if derived.exists() && !force {
                bail!(
                    "Output already exists: {} (use --force to overwrite)",
                    derived.display()
                );
            }
            if let Some(parent) = derived.parent() {
                fs::create_dir_all(parent)?;
            }
            let resized = downscale_image_bytes(&raw, max_dim, output_format)?;
            fs::write(&derived, resized)?;
            println!("Wrote {}", derived.display());
        }
    }
    Ok(())
}

fn decode_image_result(value: &str) -> Result<Vec<u8>> {
    let encoded = value
        .split_once(',')
        .filter(|(prefix, _)| prefix.starts_with("data:image/") && prefix.contains(";base64"))
        .map(|(_, payload)| payload)
        .unwrap_or(value);
    BASE64
        .decode(encoded)
        .context("failed to decode image_generation_call result")
}

fn derive_downscale_path(path: &Path, suffix: &str) -> PathBuf {
    let mut suffix = suffix.to_string();
    if !suffix.is_empty() && !suffix.starts_with('-') && !suffix.starts_with('_') {
        suffix = format!("-{suffix}");
    }
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("image");
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("png");
    path.with_file_name(format!("{stem}{suffix}.{ext}"))
}

fn downscale_image_bytes(image_bytes: &[u8], max_dim: u32, output_format: &str) -> Result<Vec<u8>> {
    if max_dim == 0 {
        bail!("--downscale-max-dim must be >= 1");
    }
    let img = image::load_from_memory(image_bytes)?;
    let scale = f32::min(1.0, max_dim as f32 / img.width().max(img.height()) as f32);
    let target_w = u32::max(1, (img.width() as f32 * scale).round() as u32);
    let target_h = u32::max(1, (img.height() as f32 * scale).round() as u32);
    let resized = if target_w == img.width() && target_h == img.height() {
        img
    } else {
        img.resize(target_w, target_h, image::imageops::FilterType::Lanczos3)
    };
    let format = match output_format {
        "png" => ImageFormat::Png,
        "jpeg" | "jpg" => ImageFormat::Jpeg,
        "webp" => ImageFormat::WebP,
        _ => bail!("unsupported output format for downscale: {output_format}"),
    };
    let final_image = if matches!(format, ImageFormat::Jpeg) {
        DynamicImage::ImageRgb8(resized.to_rgb8())
    } else {
        resized
    };
    let mut cursor = Cursor::new(Vec::new());
    final_image.write_to(&mut cursor, format)?;
    Ok(cursor.into_inner())
}

fn guess_mime_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "image/png",
    }
}

fn image_to_data_url(path: &Path) -> Result<String> {
    let bytes =
        fs::read(path).with_context(|| format!("image file not found: {}", path.display()))?;
    Ok(format!(
        "data:{};base64,{}",
        guess_mime_type(path),
        BASE64.encode(bytes)
    ))
}

fn read_jobs_jsonl(path: &Path) -> Result<Vec<BatchJob>> {
    let text = fs::read_to_string(path)
        .with_context(|| format!("input file not found: {}", path.display()))?;
    let mut jobs = Vec::new();
    for (line_no, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let value = if line.starts_with('{') {
            serde_json::from_str::<Value>(line)
                .with_context(|| format!("invalid JSON on line {}", line_no + 1))?
        } else {
            json!(line)
        };
        jobs.push(normalize_job(value, line_no + 1)?);
    }
    if jobs.is_empty() {
        bail!("No jobs found in input file");
    }
    if jobs.len() > MAX_BATCH_JOBS {
        bail!("Too many jobs ({}). Max is {}", jobs.len(), MAX_BATCH_JOBS);
    }
    Ok(jobs)
}

fn normalize_job(value: Value, idx: usize) -> Result<BatchJob> {
    match value {
        Value::String(prompt) if !prompt.trim().is_empty() => Ok(BatchJob {
            prompt: prompt.trim().to_string(),
            raw: Map::new(),
        }),
        Value::Object(mut raw) => {
            let prompt = raw
                .get("prompt")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| anyhow!("Missing prompt for job {idx}"))?
                .to_string();
            raw.insert("prompt".to_string(), json!(prompt));
            Ok(BatchJob { prompt, raw })
        }
        _ => bail!("Invalid job at index {idx}: expected string or object"),
    }
}

fn shared_for_job(base: &SharedArgs, job: &BatchJob) -> Result<SharedArgs> {
    let mut args = base.clone();
    if let Some(value) = job.raw.get("model").and_then(Value::as_str) {
        args.model = value.to_string();
    }
    if let Some(value) = job.raw.get("size").and_then(Value::as_str) {
        args.size = value.to_string();
    }
    if let Some(value) = job.raw.get("quality").and_then(Value::as_str) {
        args.quality = value.to_string();
    }
    if let Some(value) = job.raw.get("background").and_then(Value::as_str) {
        args.background = Some(value.to_string());
    }
    if let Some(value) = job
        .raw
        .get("output_format")
        .or_else(|| job.raw.get("output-format"))
        .and_then(Value::as_str)
    {
        args.output_format = Some(value.to_string());
    }
    if let Some(value) = job.raw.get("output_compression").and_then(Value::as_u64) {
        args.output_compression =
            Some(u8::try_from(value).context("output_compression must be 0..255")?);
    }
    if let Some(value) = job.raw.get("moderation").and_then(Value::as_str) {
        args.moderation = Some(value.to_string());
    }
    if let Some(value) = job.raw.get("n").and_then(Value::as_u64) {
        args.n = usize::try_from(value)?;
    }
    validate_shared(&args)?;
    Ok(args)
}

fn merged_fields(base: &Map<String, Value>, job: &BatchJob) -> Map<String, Value> {
    let mut fields = base.clone();
    if let Some(Value::Object(job_fields)) = job.raw.get("fields") {
        for (key, value) in job_fields {
            if !value.is_null() {
                fields.insert(key.clone(), value.clone());
            }
        }
    }
    for key in [
        "use_case",
        "scene",
        "subject",
        "style",
        "composition",
        "lighting",
        "palette",
        "materials",
        "text",
        "constraints",
        "negative",
    ] {
        if let Some(value) = job.raw.get(key).filter(|value| !value.is_null()) {
            fields.insert(key.to_string(), value.clone());
        }
    }
    fields
}

fn job_output_paths(
    out_dir: &Path,
    output_format: &str,
    idx: usize,
    prompt: &str,
    n: usize,
    explicit_out: Option<&str>,
) -> Result<Vec<PathBuf>> {
    let ext = format!(".{output_format}");
    let base = if let Some(explicit) = explicit_out {
        let mut path = PathBuf::from(explicit);
        if path.extension().is_none() {
            path.set_extension(output_format);
        }
        let file_name = path
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("job out must include a file name"))?;
        out_dir.join(file_name)
    } else {
        out_dir.join(format!("{idx:03}-{}{}", slugify(prompt), ext))
    };
    if n == 1 {
        return Ok(vec![base]);
    }
    let stem = base
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("image");
    let suffix = base
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{value}"))
        .unwrap_or(ext);
    Ok((1..=n)
        .map(|variant| base.with_file_name(format!("{stem}-{variant}{suffix}")))
        .collect())
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in value.chars().flat_map(char::to_lowercase).take(80) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_dash = false;
        } else if !last_dash && !slug.is_empty() {
            slug.push('-');
            last_dash = true;
        }
        if slug.len() >= 60 {
            break;
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "image".to_string()
    } else {
        slug.to_string()
    }
}

fn print_json(value: &Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}

fn path_string(path: &PathBuf) -> String {
    path.to_string_lossy().to_string()
}
