FROM node:24-bookworm AS frontend
WORKDIR /app/frontend
RUN corepack enable && corepack prepare pnpm@11.1.3 --activate
COPY frontend/package.json ./
COPY frontend/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY frontend ./
RUN pnpm run build

FROM rust:1-bookworm AS backend
WORKDIR /app
COPY Cargo.toml ./
COPY Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM debian:bookworm-slim AS pdfium
ARG TARGETARCH=amd64
ARG PDFIUM_VERSION=chromium%2F7763
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
RUN set -eux; \
    case "$TARGETARCH" in \
        amd64) archive=pdfium-linux-x64.tgz ;; \
        arm64) archive=pdfium-linux-arm64.tgz ;; \
        *) echo "unsupported TARGETARCH: $TARGETARCH" >&2; exit 1 ;; \
    esac; \
    mkdir -p /pdfium; \
    curl -fsSL "https://github.com/bblanchon/pdfium-binaries/releases/download/${PDFIUM_VERSION}/${archive}" \
        | tar -xz -C /pdfium

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
RUN groupadd --system pdf-tools \
    && useradd --system --gid pdf-tools --home-dir /app --no-create-home pdf-tools
COPY --from=backend /app/target/release/pdf-tools-server /usr/local/bin/pdf-tools-server
COPY --from=pdfium /pdfium/lib/libpdfium.so /usr/local/lib/libpdfium.so
RUN ldconfig
COPY --from=frontend /app/frontend/dist ./frontend/dist
ENV PORT=3000
ENV PDF_TOOLS_PDFIUM_PATH=/usr/local/lib/libpdfium.so
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -fsS "http://127.0.0.1:${PORT:-3000}/health" || exit 1
USER pdf-tools
CMD ["pdf-tools-server"]
