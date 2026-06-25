# syntax=docker/dockerfile:1

# Build the Svelte frontend with Node.js and pnpm.
FROM node:26-alpine AS web-builder

WORKDIR /build

ARG PNPM_VERSION=11.6.0
RUN npm install --global pnpm@${PNPM_VERSION}

COPY web/package.json web/pnpm-lock.yaml web/pnpm-workspace.yaml ./
RUN --mount=type=cache,id=folio-pnpm,target=/pnpm/store \
    pnpm install --frozen-lockfile --store-dir /pnpm/store

COPY web/ ./
RUN pnpm run build

# Build the Rust backend separately so build-only dependencies stay out of the runtime image.
FROM rust:1.96.0-trixie AS builder

RUN set -eux; \
    apt-get update; \
    DEBIAN_FRONTEND=noninteractive apt-get install -y protobuf-compiler && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /build

COPY Cargo.toml Cargo.lock ./
# Create a dummy src/main.rs to build dependencies
# This allows caching of dependencies even if source code changes
RUN --mount=type=cache,id=folio-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=folio-cargo-git,target=/usr/local/cargo/git \
    set -eux; \
    mkdir src; \
    echo "fn main() {}" > src/main.rs; \
    cargo build --release --locked; \
    rm -rf target/release/deps/folio* src

COPY . ./

RUN --mount=type=cache,id=folio-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=folio-cargo-git,target=/usr/local/cargo/git \
    set -eux; \
    cargo build --release --locked; \
    objcopy --compress-debug-sections target/release/folio ./folio

FROM debian:trixie-slim

# Install only the runtime CA bundle needed by reqwest/rustls HTTPS calls.
RUN set -eux; \
    apt-get update; \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends ca-certificates; \
    rm -rf /var/lib/apt/lists/*

ARG pkg=folio
ARG user=folio
ARG group=folio
ARG uid=10000
ARG gid=10001

# Create the runtime user and writable application directories in one layer.
RUN set -eux; \
    groupadd -g ${gid} ${group}; \
    useradd -l -u ${uid} -g ${gid} -m -s /usr/sbin/nologin ${user}; \
    install -d -o ${uid} -g ${gid} /opt/folio /opt/folio/uploads /opt/folio/data /opt/folio/tmp

USER ${user}

WORKDIR /opt/folio

COPY --from=builder --chown=${uid}:${gid} /build/folio ./folio
COPY --from=web-builder --chown=${uid}:${gid} /build/dist /opt/folio/web

ENV RUST_LOG=info
ENV FOLIO_PORT="8080"
ENV FOLIO_ADDRESS="0.0.0.0"
ENV FOLIO_WEB_PATH="/opt/folio/web"
ENV FOLIO_UPLOADS_PATH="/opt/folio/uploads"
ENV FOLIO_DATA_PATH="/opt/folio/data"

EXPOSE 8080/tcp

ENTRYPOINT ["./folio"]