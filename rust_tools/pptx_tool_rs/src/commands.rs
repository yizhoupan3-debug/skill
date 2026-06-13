// CLI command handler functions.


pub fn init_command(args: InitArgs) -> Result<()> {
    let workdir = expand_path(&args.workdir);
    let summary = init_workspace(&workdir, &args.template, args.force)?;
    emit_value(
        serde_json::to_value(summary)?,
        if args.json {
            EmitFormat::Json
        } else {
            EmitFormat::Text
        },
    )
}

pub fn outline_command(args: OutlineArgs) -> Result<()> {
    let input = expand_path(&args.input);
    let workdir = input
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    if args.bootstrap {
        init_workspace(&workdir, &args.template, false)?;
    }

    let output = expand_path(&args.output);
    let output = if output.is_absolute() {
        output
    } else {
        workdir.join(output)
    };

    let outline = read_outline(&input)?;
    let generated = generate_outline_deck_source(&outline, &args.template)?;
    fs::write(&output, generated)
        .with_context(|| format!("failed to write {}", output.display()))?;

    let mut qa_payload = None;
    let strict_quality = args.quality == QualityMode::Strict;
    if args.build || args.qa || strict_quality {
        let deck = workdir.join("deck.pptx");
        write_outline_deck_pptx(&outline, &deck, &args.template)?;
        sanitize_pptx_command(SanitizePptxArgs {
            input_path: deck.display().to_string(),
            output: None,
        })?;
    }
    if args.qa || strict_quality {
        qa_payload = Some(serde_json::to_value(qa_summary(
            &workdir.join("deck.pptx").display().to_string(),
            &workdir.join(&args.rendered_dir).display().to_string(),
        )?)?);
        if strict_quality {
            strict_quality_gate(qa_payload.as_ref().unwrap())?;
        }
    }

    emit_value(
        serde_json::to_value(OutlineSummary {
            input: input.display().to_string(),
            output: output.display().to_string(),
            bootstrapped: args.bootstrap,
            built: args.build || args.qa || strict_quality,
            qa: qa_payload,
        })?,
        if args.json {
            EmitFormat::Json
        } else {
            EmitFormat::Text
        },
    )
}

pub fn qa_command(args: QaArgs) -> Result<()> {
    qa::qa_command(args)
}

pub fn intake_command(args: IntakeArgs) -> Result<()> {
    let structure = extract_structure_payload(&args.deck)?;
    let inspector = office::office_doctor_value(&args.deck)?;
    let payload = json!({
        "deck": args.deck,
        "structure": structure,
        "inspector": inspector,
    });
    emit_value(
        payload,
        if args.json {
            EmitFormat::Json
        } else {
            EmitFormat::Text
        },
    )
}

pub fn build_qa_command(args: BuildQaArgs) -> Result<()> {
    let workdir = expand_path(&args.workdir);
    let entry = expand_path(&args.entry);
    let entry = if entry.is_absolute() {
        entry
    } else {
        workdir.join(entry)
    };
    let outline = read_outline(&entry)?;
    let deck = workdir.join(&args.deck);
    let deck_template = deck_template_from_outline(&outline);
    write_outline_deck_pptx(&outline, &deck, &deck_template)?;
    sanitize_pptx_command(SanitizePptxArgs {
        input_path: deck.display().to_string(),
        output: None,
    })?;
    let rendered = workdir.join(&args.rendered_dir);
    let payload = qa_summary(&deck.display().to_string(), &rendered.display().to_string())?;
    if args.quality == QualityMode::Strict {
        strict_quality_gate(&serde_json::to_value(&payload)?)?;
    }
    emit_value(
        serde_json::to_value(payload)?,
        if args.json {
            EmitFormat::Json
        } else {
            EmitFormat::Text
        },
    )
}

pub fn office_command(args: OfficeArgs) -> Result<()> {
    office::office_command(args)
}

pub fn office_probe_command(args: OfficeProbeArgs) -> Result<()> {
    office::office_probe_command(args)
}

pub fn office_doctor_command(args: OfficeDoctorArgs) -> Result<()> {
    office::office_doctor_command(args)
}

pub fn office_file_passthrough(
    command: &str,
    file: &str,
    tail: Option<&str>,
    json_output: bool,
) -> Result<()> {
    office::office_file_passthrough(command, file, tail, json_output)
}

pub fn office_get_command(args: OfficeGetArgs) -> Result<()> {
    office::office_get_command(args)
}

pub fn office_query_command(args: OfficeQueryArgs) -> Result<()> {
    office::office_query_command(args)
}

pub fn office_watch_command(args: OfficeWatchArgs) -> Result<()> {
    office::office_watch_command(args)
}

pub fn office_batch_command(args: OfficeBatchArgs) -> Result<()> {
    office::office_batch_command(args)
}

pub fn render_command(args: RenderArgs) -> Result<()> {
    let input = expand_path(&args.input_path);
    let output_dir = args
        .output_dir
        .as_deref()
        .map(expand_path)
        .unwrap_or_else(|| default_render_dir(&input));
    let rendered = render_paths(&input, &output_dir, args.width, args.height)?;
    for path in rendered {
        println!("{}", path.display());
    }
    Ok(())
}
