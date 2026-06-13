use super::*;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::{SystemTime, UNIX_EPOCH};
    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;
    use zip::ZipWriter;

    fn temp_xlsx_path(name: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("{}_{}.xlsx", name, unique));
        path
    }

    fn write_zip_entry<W: Write + Seek>(zip: &mut ZipWriter<W>, path: &str, content: &[u8]) {
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        zip.start_file(path, options).unwrap();
        zip.write_all(content).unwrap();
    }

    fn build_test_workbook(path: &Path) {
        let file = File::create(path).unwrap();
        let mut zip = ZipWriter::new(file);

        write_zip_entry(
            &mut zip,
            "[Content_Types].xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
</Types>"#,
        );
        write_zip_entry(
            &mut zip,
            "xl/workbook.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
          xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Visible" sheetId="1" r:id="rId1"/>
    <sheet name="Hidden" sheetId="2" state="hidden" r:id="rId2"/>
  </sheets>
  <definedNames>
    <definedName name="_xlnm.Print_Area" localSheetId="0">'Visible'!$A$1:$C$4</definedName>
    <definedName name="LocalRange" hidden="1">Visible!$A$2:$A$3</definedName>
  </definedNames>
</workbook>"#,
        );
        write_zip_entry(
            &mut zip,
            "xl/_rels/workbook.xml.rels",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/>
</Relationships>"#,
        );
        write_zip_entry(
            &mut zip,
            "xl/worksheets/sheet1.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
           xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <dimension ref="A1:C4"/>
  <sheetViews>
    <sheetView workbookViewId="0">
      <pane xSplit="1" ySplit="1" topLeftCell="B2" state="frozen"/>
    </sheetView>
  </sheetViews>
  <sheetData>
    <row r="1">
      <c r="A1" t="inlineStr"><is><t>Name</t></is></c>
      <c r="B1"><f>SUM(A2:A3)</f></c>
    </row>
    <row r="2">
      <c r="A2"><v>1</v></c>
    </row>
    <row r="3">
      <c r="A3"><f>A2*2</f></c>
    </row>
  </sheetData>
  <autoFilter ref="A1:C4"/>
  <mergeCells count="1">
    <mergeCell ref="A1:A2"/>
  </mergeCells>
  <conditionalFormatting sqref="B2:B4">
    <cfRule type="expression" priority="1"><formula>1</formula></cfRule>
  </conditionalFormatting>
  <dataValidations count="1">
    <dataValidation type="whole" sqref="C2:C4"/>
  </dataValidations>
  <drawing r:id="rId2"/>
  <tableParts count="1">
    <tablePart r:id="rId1"/>
  </tableParts>
</worksheet>"#,
        );
        write_zip_entry(
            &mut zip,
            "xl/worksheets/_rels/sheet1.xml.rels",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/table" Target="../tables/table1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing" Target="../drawings/drawing1.xml"/>
</Relationships>"#,
        );
        write_zip_entry(
            &mut zip,
            "xl/worksheets/sheet2.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <dimension ref="A1:A1"/>
  <sheetData>
    <row r="1"><c r="A1"><v>42</v></c></row>
  </sheetData>
</worksheet>"#,
        );
        write_zip_entry(
            &mut zip,
            "xl/tables/table1.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<table xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"
       id="1"
       name="Table1"
       displayName="Table1"
       ref="A1:C4"/>"#,
        );
        write_zip_entry(
            &mut zip,
            "xl/drawings/drawing1.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing"/>"#,
        );
        write_zip_entry(
            &mut zip,
            "xl/drawings/_rels/drawing1.xml.rels",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart" Target="../charts/chart1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image1.png"/>
</Relationships>"#,
        );
        write_zip_entry(
            &mut zip,
            "xl/charts/chart1.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?><chartSpace xmlns="http://schemas.openxmlformats.org/drawingml/2006/chart"/>"#,
        );
        write_zip_entry(&mut zip, "xl/media/image1.png", b"not-a-real-png");
        write_zip_entry(
            &mut zip,
            "xl/externalLinks/externalLink1.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?><externalLink xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"/>"#,
        );

        zip.finish().unwrap();
    }

    fn temp_docx_path(name: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("{}_{}.docx", name, unique));
        path
    }

    fn build_test_document(path: &Path) {
        let file = File::create(path).unwrap();
        let mut zip = ZipWriter::new(file);

        write_zip_entry(
            &mut zip,
            "[Content_Types].xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
</Types>"#,
        );
        write_zip_entry(
            &mut zip,
            "word/document.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:body>
    <w:p>
      <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
      <w:r><w:t>Executive Summary</w:t></w:r>
    </w:p>
    <w:p><w:r><w:t>Body text</w:t></w:r></w:p>
    <w:tbl><w:tr><w:tc><w:p><w:r><w:t>Cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
    <w:p><w:hyperlink r:id="rIdHyper"><w:r><w:t>Link</w:t></w:r></w:hyperlink></w:p>
    <w:p><w:r><w:drawing/></w:r></w:p>
    <w:sectPr><w:pgSz w:w="12240" w:h="15840"/></w:sectPr>
  </w:body>
</w:document>"#,
        );
        write_zip_entry(
            &mut zip,
            "word/_rels/document.xml.rels",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdImage" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>
  <Relationship Id="rIdHyper" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com"/>
</Relationships>"#,
        );
        write_zip_entry(
            &mut zip,
            "word/footnotes.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:footnote w:id="1"><w:p/></w:footnote>
</w:footnotes>"#,
        );
        write_zip_entry(
            &mut zip,
            "word/comments.xml",
            br#"<?xml version="1.0" encoding="UTF-8"?>
<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:comment w:id="0"><w:p/></w:comment>
</w:comments>"#,
        );
        write_zip_entry(&mut zip, "word/media/image1.png", b"not-a-real-png");

        zip.finish().unwrap();
    }

    #[test]
    fn inspect_xlsx_summary_preserves_workbook_structure() {
        let path = temp_xlsx_path("ooxml_parser_rs_fixture");
        build_test_workbook(&path);

        let summary = inspect_xlsx_summary(&path).unwrap();
        std::fs::remove_file(&path).unwrap();

        assert_eq!(summary.sheet_count, 2);
        assert_eq!(summary.sheet_names, vec!["Visible", "Hidden"]);
        assert_eq!(summary.external_link_count, 1);
        assert_eq!(summary.defined_names.len(), 2);
        assert_eq!(summary.sheets[0].state, "visible");
        assert_eq!(summary.sheets[0].dimensions, "A1:C4");
        assert_eq!(summary.sheets[0].size_index, "1:4 x 1:3");
        assert_eq!(summary.sheets[0].freeze_panes.as_deref(), Some("B2"));
        assert_eq!(summary.sheets[0].auto_filter.as_deref(), Some("A1:C4"));
        assert_eq!(
            summary.sheets[0].print_area.as_deref(),
            Some("'Visible'!$A$1:$C$4")
        );
        assert_eq!(summary.sheets[0].formula_count, 2);
        assert_eq!(summary.sheets[0].merged_ranges, 1);
        assert_eq!(summary.sheets[0].data_validation_rules, 1);
        assert_eq!(summary.sheets[0].conditional_format_regions, 1);
        assert_eq!(summary.sheets[0].chart_count, 1);
        assert_eq!(summary.sheets[0].image_count, 1);
        assert_eq!(summary.sheets[0].tables.len(), 1);
        assert_eq!(summary.sheets[0].tables[0].name.as_deref(), Some("Table1"));
        assert_eq!(
            summary.sheets[0].tables[0].reference.as_deref(),
            Some("A1:C4")
        );
        assert_eq!(summary.sheets[1].state, "hidden");
        assert_eq!(summary.sheets[1].dimensions, "A1:A1");
    }

    #[test]
    fn inspect_docx_summary_reports_document_structure() {
        let path = temp_docx_path("ooxml_parser_rs_docx_fixture");
        build_test_document(&path);

        let summary = inspect_docx_summary(&path).unwrap();
        std::fs::remove_file(&path).unwrap();

        assert_eq!(summary.paragraph_count, 5);
        assert_eq!(summary.heading_count, 1);
        assert_eq!(summary.headings[0].level, Some(1));
        assert_eq!(summary.headings[0].text, "Executive Summary");
        assert_eq!(summary.table_count, 1);
        assert_eq!(summary.section_count, 1);
        assert_eq!(summary.image_count, 1);
        assert_eq!(summary.hyperlink_count, 1);
        assert_eq!(summary.footnote_count, 1);
        assert_eq!(summary.comment_count, 1);
        let page_size = summary.page_size.unwrap();
        assert_eq!(page_size.width_inches, 8.5);
        assert_eq!(page_size.height_inches, 11.0);
    }

    #[test]
    fn read_docx_emits_linear_body_content() {
        let path = temp_docx_path("ooxml_parser_rs_read_docx_fixture");
        build_test_document(&path);

        let output = read_docx_content(&path).unwrap();
        std::fs::remove_file(&path).unwrap();

        assert!(output
            .blocks
            .iter()
            .any(|block| matches!(block, DocxBlock::Paragraph { text, heading_level: Some(1) } if text == "Executive Summary")));
        assert!(output.blocks.iter().any(
            |block| matches!(block, DocxBlock::Paragraph { text, heading_level: None } if text == "Body text")
        ));
        assert!(output
            .blocks
            .iter()
            .any(|block| matches!(block, DocxBlock::Table { rows } if rows[0][0] == "Cell")));
        assert!(output.blocks.iter().any(|block| matches!(block, DocxBlock::Image)));
    }

    #[test]
    fn read_xlsx_emits_sheet_rows() {
        let path = temp_xlsx_path("ooxml_parser_rs_read_xlsx_fixture");
        build_test_workbook(&path);

        let output = read_xlsx_content(&path, 10_000, &["Visible".to_string()]).unwrap();
        std::fs::remove_file(&path).unwrap();

        assert_eq!(output.sheets.len(), 1);
        assert_eq!(output.sheets[0].name, "Visible");
        assert!(!output.sheets[0].rows.is_empty());
        assert_eq!(output.sheets[0].rows[0][0], "Name");
    }

    #[test]
    fn parse_dimension_handles_single_cells_and_ranges() {
        let single = parse_dimension("B3").unwrap();
        assert_eq!(single.min_row, 3);
        assert_eq!(single.max_col, 2);

        let range = parse_dimension("$C$2:$F$9").unwrap();
        assert_eq!(range.min_col, 3);
        assert_eq!(range.max_row, 9);
        assert_eq!(format_dimension(range), "C2:F9");
    }
}
