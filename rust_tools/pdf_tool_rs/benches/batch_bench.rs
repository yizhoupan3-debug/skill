//! Batch throughput bench (opt-in).
//!
//! ```bash
//! PDF_BENCH=1 cargo bench -p pdf_tool_rs --bench batch_bench
//! # quick smoke (fewer samples):
//! PDF_BENCH=1 cargo bench -p pdf_tool_rs --bench batch_bench -- --sample-size 10
//! ```
//!
//! Without `PDF_BENCH=1` the binary exits immediately (CI-friendly no-op).

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use pdf_tool_rs::batch::{BatchOptions, run_batch};
use tempfile::tempdir;

fn hello_pdf_bytes() -> Vec<u8> {
    use lopdf::content::{Content, Operation};
    use lopdf::{Document, Object, Stream, dictionary};
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Courier",
    });
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
    });
    let content = Content {
        operations: vec![
            Operation::new("BT", vec![]),
            Operation::new("Tf", vec!["F1".into(), 24.into()]),
            Operation::new("Td", vec![100.into(), 600.into()]),
            Operation::new("Tj", vec![Object::string_literal("Hello World!")]),
            Operation::new("ET", vec![]),
        ],
    };
    let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => content_id,
        "Resources" => resources_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    });
    let pages = dictionary! {
        "Type" => "Pages",
        "Count" => 1,
        "Kids" => vec![page_id.into()],
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages));
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);
    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("save hello pdf");
    buf
}

fn bench_batch_small(c: &mut Criterion) {
    let pdf_bytes = hello_pdf_bytes();
    c.bench_function("batch_4_pdfs_jobs_2", |b| {
        b.iter(|| {
            let tmp = tempdir().expect("tempdir");
            let mut paths = Vec::new();
            for i in 0..4 {
                let p = tmp.path().join(format!("doc_{i}.pdf"));
                std::fs::write(&p, &pdf_bytes).expect("write pdf");
                paths.push(p);
            }
            let out = tmp.path().join("out");
            let opts = BatchOptions {
                out_dir: out,
                jobs: 2,
                resume: false,
                fail_fast: false,
                max_chars: 8000,
            };
            black_box(run_batch(paths, &opts, false).expect("batch"));
        });
    });
}

criterion_group!(benches, bench_batch_small);

fn main() {
    if std::env::var("PDF_BENCH").ok().as_deref() != Some("1") {
        eprintln!("skip batch_bench (set PDF_BENCH=1 to run)");
        return;
    }
    criterion_main!(benches);
}
