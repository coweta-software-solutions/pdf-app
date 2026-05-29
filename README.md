# PDF Tools

Simple self-hosted PDF tools app with a Rust backend and Svelte SPA frontend.

## Features

- Image to PDF
- PDF to image with single page, ranges, or all pages
- Merge PDFs
- Split/extract PDF pages

## Stack

- Rust + axum backend
- Svelte frontend managed with Vite Plus (`vp`)
- PDFium for PDF rendering, merge, and split
- `image` + `printpdf` for image-to-PDF work

## Project Layout

```text
src/main.rs        Axum routes and server startup
src/state.rs       Shared app state, config, PDFium binding, CPU worker limiter
src/job_store.rs   In-memory async job records and downloadable job results
src/download.rs    Download response formatting
src/convert.rs     Image/PDF conversion operations
src/pdf_ops.rs     PDF merge and split operations
src/upload.rs      Multipart upload parsing and file validation
frontend/src/      Svelte UI and browser API client
```

## Run With Docker Compose

```sh
docker compose up --build
```

Open `http://localhost:3000`.

## Local Development

Copy `.env.example` if you want a local configuration file, or export the
variables directly in your shell.

Backend:

```sh
export PDF_TOOLS_PDFIUM_PATH=/path/to/libpdfium.so # optional if libpdfium.so is on the system library path
cargo run
```

Frontend:

```sh
cd frontend
vp install
vp dev
```

The frontend dev server proxies API requests to `http://127.0.0.1:3000`.

## Configuration

```text
PORT=3000
MAX_UPLOAD_MB=100
MAX_RENDER_PAGES=100
PDF_TOOLS_PDFIUM_PATH=/path/to/libpdfium.so
PDF_TOOLS_CPU_PERMITS=min(host_parallelism, 4)
```

`PDF_TOOLS_PDFIUM_PATH` is optional when `libpdfium.so` can be found through the
system library path. The Docker image includes PDFium and sets this path for you.

## Checks

```sh
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test

cd frontend
vp check
vp build
```

The GitHub Actions workflow in `.github/workflows/ci.yml` runs the same backend
and frontend checks on pushes and pull requests.

## API

```text
GET  /health
POST /convert
POST /merge
POST /split
POST /jobs
GET  /jobs/{id}
GET  /jobs/{id}/download
```

All operation endpoints accept `multipart/form-data` and return a direct download.
