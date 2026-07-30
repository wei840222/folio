# syntax=docker/dockerfile:1

# Build the Svelte frontend with Node.js and pnpm.
FROM node:26-alpine AS web-builder

WORKDIR /build

ARG PNPM_VERSION=11.6.0
RUN npm install --global pnpm@${PNPM_VERSION}

COPY web/package.json web/pnpm-lock.yaml web/pnpm-workspace.yaml ./
RUN --mount=type=cache,id=agenfact-pnpm,target=/pnpm/store \
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
RUN --mount=type=cache,id=agenfact-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=agenfact-cargo-git,target=/usr/local/cargo/git \
    set -eux; \
    mkdir src; \
    echo "fn main() {}" > src/main.rs; \
    cargo build --release --locked; \
    rm -rf target/release/deps/agenfact* src

COPY . ./

RUN --mount=type=cache,id=agenfact-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=agenfact-cargo-git,target=/usr/local/cargo/git \
    set -eux; \
    cargo build --release --locked; \
    objcopy --compress-debug-sections target/release/agenfact ./agenfact

FROM debian:trixie-slim

# Install only the runtime CA bundle needed by reqwest/rustls HTTPS calls.
RUN set -eux; \
    apt-get update; \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends ca-certificates; \
    rm -rf /var/lib/apt/lists/*

ARG pkg=agenfact
ARG user=agenfact
ARG group=agenfact
ARG uid=10000
ARG gid=10001

# Create the runtime user and writable application directories in one layer.
RUN set -eux; \
    groupadd -g ${gid} ${group}; \
    useradd -l -u ${uid} -g ${gid} -m -s /usr/sbin/nologin ${user}; \
    install -d -o ${uid} -g ${gid} /opt/agenfact /opt/agenfact/uploads /opt/agenfact/data /opt/agenfact/tmp

USER ${user}

WORKDIR /opt/agenfact

COPY --from=builder --chown=${uid}:${gid} /build/agenfact ./agenfact
COPY --from=web-builder --chown=${uid}:${gid} /build/dist /opt/agenfact/web

ENV RUST_LOG=info
ENV AGENFACT_PORT="8080"
ENV AGENFACT_ADDRESS="0.0.0.0"
ENV AGENFACT_WEB_PATH="/opt/agenfact/web"
ENV AGENFACT_UPLOADS_PATH="/opt/agenfact/uploads"
ENV AGENFACT_DATA_PATH="/opt/agenfact/data"

EXPOSE 8080/tcp

ENTRYPOINT ["./agenfact"]