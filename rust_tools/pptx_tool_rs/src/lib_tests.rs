use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_slide_range_single_and_span() {
        assert_eq!(parse_slide_range("3", 10).unwrap(), vec![3]);
        assert_eq!(parse_slide_range("1-3", 10).unwrap(), vec![1, 2, 3]);
        assert!(parse_slide_range("0", 5).is_err());
        assert!(parse_slide_range("11", 10).is_err());
        assert!(parse_slide_range("3-2", 10).is_err());
    }

    #[test]
    fn format_read_full_text_linear_sections() {
        let structure = json!({
            "file": "demo.pptx",
            "slide_count": 1,
            "slides": [{
                "index": 0,
                "layout": "Title Slide",
                "elements": [
                    {"name": "Title 1", "type": "shape", "text": {"fullText": "Hello Deck"}},
                    {"name": "Content 1", "type": "shape", "text": {"fullText": "Point A"}},
                    {"name": "Picture 1", "type": "image"}
                ],
                "notes": "Remember to pause"
            }]
        });
        let text = format_read_full_text(&structure);
        assert!(text.contains("=== Slide 1 ==="));
        assert!(text.contains("TITLE:\nHello Deck"));
        assert!(text.contains("BODY:\nPoint A"));
        assert!(text.contains("NOTES:\nRemember to pause"));
        assert!(text.contains("image \"Picture 1\" (no extractable text)"));
    }

    #[test]
    fn parse_pdf_page_size_points() {
        let (w, h) = parse_pdf_page_size("612 x 792 pts (letter)").unwrap();
        assert_eq!(w, 612.0);
        assert_eq!(h, 792.0);
    }

    #[test]
    fn normalize_font_names() {
        assert_eq!(
            normalize_font_family_name("Helvetica Neue (Body)"),
            "helvetica neue"
        );
        assert_eq!(normalize_font_family_name("PingFang-SC"), "pingfang sc");
    }

    #[test]
    fn sanitize_presentation_xml_reorders_notes_master_after_slide_master() {
        let input = r#"<p:presentation><p:sldMasterIdLst/><p:sldIdLst/><p:notesMasterIdLst><p:notesMasterId r:id="rId4"/></p:notesMasterIdLst><p:sldSz cx="1" cy="2"/><p:notesSz cx="2" cy="1"/><p:defaultTextStyle/></p:presentation>"#;
        let output = sanitize_presentation_xml(input).unwrap();
        assert!(
            output.find("<p:sldMasterIdLst/>").unwrap()
                < output.find("<p:notesMasterIdLst>").unwrap()
        );
        assert!(
            output.find("<p:notesMasterIdLst>").unwrap() < output.find("<p:sldIdLst/>").unwrap()
        );
    }

    #[test]
    fn outline_source_embeds_design_brief() {
        let outline = json!({
            "title": "测试汇报",
            "slides": [
                {"title": "本页展示增长路径", "bullets": ["赋能业务", "具有重要意义"]}
            ]
        });
        let source = generate_outline_deck_source(&outline, &DeckTemplate::Dark).unwrap();
        assert!(source.contains("ppt-rust-outline-plan"));
        assert!(source.contains("built-in Rust copy naturalization"));
        assert!(source.contains("$copywriting"));
        assert!(source.contains("$paper-writing"));
        assert!(source.contains("design-md drift verdict"));
        assert!(!source.contains("本页展示增长路径"));
        assert!(source.contains("增长路径"));
        assert!(source.contains("支持业务"));
        assert!(source.contains("会影响具体决策"));
    }

    #[test]
    fn strict_quality_gate_accepts_rust_inspector_and_rejects_overflow() {
        let clean = json!({
            "overflow_check": {"ok": true},
            "overlap_check": {"ok": true},
            "aesthetic_check": {"ok": true, "failing_slides": []},
            "font_check": {"ok": true},
            "inspector": {"validation": {"ok": true}, "issues": {"count": 0}}
        });
        strict_quality_gate(&clean).unwrap();

        let overflow = json!({
            "overflow_check": {"ok": false},
            "overlap_check": {"ok": true},
            "aesthetic_check": {"ok": true, "failing_slides": []},
            "font_check": {"ok": true},
            "inspector": {"validation": {"ok": true}, "issues": {"count": 0}}
        });
        assert!(strict_quality_gate(&overflow).is_err());

        let overlap = json!({
            "overflow_check": {"ok": true},
            "overlap_check": {"ok": false},
            "aesthetic_check": {"ok": true, "failing_slides": []},
            "font_check": {"ok": true},
            "inspector": {"validation": {"ok": true}, "issues": {"count": 0}}
        });
        assert!(strict_quality_gate(&overlap).is_err());

        let aesthetic = json!({
            "overflow_check": {"ok": true},
            "overlap_check": {"ok": true},
            "aesthetic_check": {"ok": false, "failing_slides": [2, 4]},
            "font_check": {"ok": true},
            "inspector": {"validation": {"ok": true}, "issues": {"count": 0}}
        });
        assert!(strict_quality_gate(&aesthetic).is_err());

        let missing_overflow_field = json!({
            "overlap_check": {"ok": true},
            "aesthetic_check": {"ok": true, "failing_slides": []},
            "font_check": {"ok": true},
            "inspector": {"validation": {"ok": true}, "issues": {"count": 0}}
        });
        assert!(strict_quality_gate(&missing_overflow_field).is_err());
    }
}
