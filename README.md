# PDF Tools

Small self-hosted PDF tools app with a Rust backend and Svelte SPA frontend.
The published container image includes the frontend, API, and PDFium runtime in
one service.

## Features

- Convert PNG or JPEG images into a PDF
- Export PDF pages as PNG or JPEG images
- Merge multiple PDFs
- Extract selected PDF pages into a new PDF

## Deploy With Docker Compose

Requirements:

- Docker with the Compose plugin
- Access to `ghcr.io`

Clone the repository or download `docker-compose.yml`, then start the app:

```sh
docker compose up -d
```

Open `http://localhost:3000`.

The default Compose file pulls:

```text
ghcr.io/coweta-software-solutions/pdf-app:latest
```

The service runs as a non-root user inside the container and has a built-in
health check at `/health`.

## Standalone Compose File

If you do not want to clone the repository, create a `docker-compose.yml` with:

```yaml
services:
  pdf-tools:
    image: ghcr.io/coweta-software-solutions/pdf-app:latest
    ports:
      - "${PORT:-3000}:${PORT:-3000}"
    environment:
      PORT: "${PORT:-3000}"
      MAX_UPLOAD_MB: "${MAX_UPLOAD_MB:-100}"
      MAX_RENDER_PAGES: "${MAX_RENDER_PAGES:-100}"
      PDF_TOOLS_CPU_PERMITS: "${PDF_TOOLS_CPU_PERMITS:-4}"
    restart: unless-stopped
```

Then run:

```sh
docker compose up -d
```

## Configuration

Copy `.env.example` to `.env` next to `docker-compose.yml` to customize the
self-hosted container defaults:

```text
PORT=3000
MAX_UPLOAD_MB=100
MAX_RENDER_PAGES=100
PDF_TOOLS_CPU_PERMITS=4
```

- `PORT`: HTTP port the server listens on.
- `MAX_UPLOAD_MB`: maximum accepted multipart upload size.
- `MAX_RENDER_PAGES`: maximum pages rendered from a PDF conversion request.
- `PDF_TOOLS_CPU_PERMITS`: number of CPU-bound PDF jobs allowed at once. If unset,
  the app uses `min(host_parallelism, 4)`.

The Docker image bundles PDFium and sets `PDF_TOOLS_PDFIUM_PATH` itself. Local
non-Docker runs only need `PDF_TOOLS_PDFIUM_PATH` when `libpdfium.so` is not
available on the system library path.

The app does not require a persistent volume. Jobs and generated downloads are
kept in memory and are cleared when the container restarts.

## Upgrade

Pull the newest image and restart the service:

```sh
docker compose pull
docker compose up -d
```

## Pin Or Roll Back

For production use, prefer a pinned release tag instead of `latest`:

```yaml
image: ghcr.io/coweta-software-solutions/pdf-app:v0.1.0
```

For rollback, replace the image tag with the immutable `sha-<shortsha>` tag from
the GitHub Actions run or release you want to restore, then run:

```sh
docker compose pull
docker compose up -d
```

## Run With Docker

Use this path if you do not want Docker Compose:

```sh
docker run --rm -p 3000:3000 ghcr.io/coweta-software-solutions/pdf-app:latest
```

## Build From Source

Use this path for local source builds or when you do not want to pull the
published GHCR image:

```sh
cp .env.example .env
docker compose -f docker-compose.build.yml up --build
```

Open `http://localhost:3000`.

## Develop Locally

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
and frontend checks on pushes and pull requests. The workflow in
`.github/workflows/container.yml` publishes multi-architecture images to GHCR on
pushes to `main` or `master`, version tags such as `v0.1.0`, and manual runs.

## Maintainer Release Notes

After the first GHCR publish, the package may need to be marked public in the
GitHub package settings if repository/package visibility does not already expose
it.
