# PDF Tools

Simple self-hosted PDF tools app with a Rust backend and Svelte SPA frontend.

## What It Does

- Convert PNG or JPEG images into a PDF
- Export PDF pages as PNG or JPEG images
- Merge multiple PDFs
- Extract selected PDF pages into a new PDF

## Quick Start With Docker Compose

```sh
docker compose up --build
```

Open `http://localhost:3000`.

The Compose service builds the app image locally, serves the frontend and API from
one container, and restarts with `unless-stopped`.

## Run With Docker

Use this path if you do not want Docker Compose.

```sh
cp .env.example .env
docker build -t pdf-tools .
docker run --rm -p 3000:3000 --env-file .env pdf-tools
```

Open `http://localhost:3000`.

The image includes PDFium and sets `PDF_TOOLS_PDFIUM_PATH` for the bundled
library. If the default configuration is enough, you can omit `--env-file .env`.

## Configuration

```text
PORT=3000
MAX_UPLOAD_MB=100
MAX_RENDER_PAGES=100
PDF_TOOLS_CPU_PERMITS=4
PDF_TOOLS_PDFIUM_PATH=/path/to/libpdfium.so
```

- `PORT`: HTTP port the server listens on.
- `MAX_UPLOAD_MB`: maximum accepted multipart upload size.
- `MAX_RENDER_PAGES`: maximum pages rendered from a PDF conversion request.
- `PDF_TOOLS_CPU_PERMITS`: number of CPU-bound PDF jobs allowed at once. If unset,
  the app uses `min(host_parallelism, 4)`.
- `PDF_TOOLS_PDFIUM_PATH`: path to `libpdfium.so`. Docker sets this to the bundled
  PDFium library. Local non-Docker runs only need it when PDFium is not available
  on the system library path.

## Build And Run Locally

Copy `.env.example` if you want a local configuration file, or export the
variables directly in your shell.

Backend:

```sh
cargo run
```

If `libpdfium.so` is not on the system library path, set:

```sh
export PDF_TOOLS_PDFIUM_PATH=/path/to/libpdfium.so
```

Frontend:

```sh
cd frontend
vp install
vp dev
```

The frontend dev server proxies API requests to `http://127.0.0.1:3000`.

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
