# use the official Bun image
# see all versions at https://hub.docker.com/r/oven/bun/tags
FROM oven/bun:1.3.2-debian AS web-builder

WORKDIR /build

# skip optional deps (e.g. sharp) to speed up install
ENV npm_config_optional=false
COPY ./web/package.json ./web/bun.lock ./
RUN bun install --frozen-lockfile

COPY ./web/ ./
RUN bun dist

FROM rust:1.91.1-trixie AS builder

RUN set -x && apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    protobuf-compiler && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /build

COPY Cargo.toml Cargo.lock ./
# Create a dummy src/main.rs to build dependencies
# This allows caching of dependencies even if source code changes
RUN set -eux; \
    mkdir src; \
    echo "fn main() {}" > src/main.rs; \
    cargo build --release;\
    rm -rf target/release/deps/folio* src

COPY . ./

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    set -eux; \
    cargo build --release; \
    objcopy --compress-debug-sections target/release/folio ./folio

FROM debian:trixie-slim

# update ca-certificates
RUN set -x && apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    ca-certificates && \
    rm -rf /var/lib/apt/lists/*

ARG pkg=folio
ARG user=folio
ARG group=folio
ARG uid=10000
ARG gid=10001

# create user
RUN groupadd -g ${gid} ${group} && \
    useradd -l -u ${uid} -g ${gid} -m -s /bin/bash ${user}

USER ${user}

WORKDIR /opt/folio

COPY --from=builder /build/folio ./folio
COPY --from=web-builder --chown=${uid}:${gid} /build/dist /opt/folio/web
RUN mkdir -p /opt/folio/uploads && chown ${uid}:${gid} /opt/folio/uploads
RUN mkdir -p /opt/folio/tmp && chown ${uid}:${gid} /opt/folio/tmp

ENV RUST_LOG=info
ENV ROCKET_CLI_COLOR="true"
ENV ROCKET_PORT="8080"
ENV ROCKET_ADDRESS="0.0.0.0"
ENV ROCKET_LIMITS='{file="5 MiB"}'
ENV ROCKET_TMP_DIR="/opt/folio/tmp"
ENV FOLIO_WEB_PATH="/opt/folio/web"
ENV FOLIO_UPLOADS_PATH="/opt/folio/uploads"

EXPOSE 8080/tcp

ENTRYPOINT ["./folio"]