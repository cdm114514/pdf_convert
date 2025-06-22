# PDF Convert Example

This project demonstrates extracting text from a PDF and writing a new PDF using Rust.

## Build

```bash
cargo build --release
```

## Run Example

```bash
PDFIUM_LIB_PATH=$(pwd)/lib cargo run -- latex_input.pdf typst_output.pdf
```

- `latex_input.pdf`: Input PDF file
- `typst_output.pdf`: Output PDF file (generated)

## Requirements
- Rust
- PDFium library (provided in `lib/`) 