# ==============================================================================
# Signline v0.1.0b (Beta) Multi-Stage Dockerfile (Optional Docker Deployment)
# Stack: Rust (Axum/Tokio) + PocketBase + TypeScript (React 18 / ShadCN/UI)
# Build from repository root:
#   docker build -f docker/Dockerfile -t signline/signline:0.1.0b .
# ==============================================================================

FROM node:20-bookworm-slim AS frontend-builder
WORKDIR /app/frontend
COPY frontend/package*.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

FROM rust:1-bookworm AS backend-builder
WORKDIR /app
COPY backend/ ./backend/
COPY build_script/tui-installer/ ./build_script/tui-installer/
RUN cargo build --release --manifest-path backend/Cargo.toml && \
    cargo build --release --manifest-path build_script/tui-installer/Cargo.toml

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    unzip \
    poppler-utils \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /opt/signline
RUN mkdir -p /opt/signline/bin /opt/signline/pb_data && \
    curl -sL "https://github.com/pocketbase/pocketbase/releases/download/v0.25.9/pocketbase_0.25.9_linux_amd64.zip" -o /tmp/pb.zip && \
    unzip -o /tmp/pb.zip pocketbase -d /opt/signline/bin/ && \
    chmod +x /opt/signline/bin/pocketbase && \
    rm -f /tmp/pb.zip

COPY .version VERSION LICENSE LICENSE_ADDITIONAL_TERMS README.md CHANGELOG.md ./
COPY public/ ./public/
COPY pb_migrations/ ./pb_migrations/
COPY --from=frontend-builder /app/frontend/dist ./frontend/dist
COPY --from=backend-builder /app/backend/target/release/signline-backend /usr/bin/signline-server
COPY --from=backend-builder /app/build_script/tui-installer/target/release/signline-installer /usr/bin/signline-installer

ENV SIGNLINE_ROOT=/opt/signline \
    HOST=0.0.0.0 \
    PORT=3000 \
    POCKETBASE_URL=http://127.0.0.1:8090 \
    POCKETBASE_BIN=/opt/signline/bin/pocketbase \
    POCKETBASE_DATA_DIR=/opt/signline/pb_data

EXPOSE 3000
VOLUME ["/opt/signline/pb_data"]

CMD ["/usr/bin/signline-server"]
