# pdf_tool_rs

Pure Rust PDF text extraction CLI (`pdf`) and batch catalog writer.

## Tests

```bash
cargo test -p pdf_tool_rs
```

Integration fixtures are generated in `tests/fixtures.rs` (hello text PDF, blank pages, image-only scanned proxy).

## Benchmarks (opt-in)

Batch throughput bench is gated on `PDF_BENCH=1` so CI/default `cargo bench` stays a no-op:

```bash
PDF_BENCH=1 cargo bench -p pdf_tool_rs --bench batch_bench
PDF_BENCH=1 cargo bench -p pdf_tool_rs --bench batch_bench -- --sample-size 10
```

HTML reports land under `target/criterion/`. See also `skills/pdf/references/detailed-guide.md` (batch / `content_class` / `--skip-scanned`).
