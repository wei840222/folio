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

WORKDIR /build

ARG pkg=folio

COPY Cargo.toml Cargo.lock ./
# Create a dummy src/main.rs to build dependencies
# This allows caching of dependencies even if source code changes
RUN set -eux; \
    mkdir src; \
    echo "fn main() {}" > src/main.rs; \
    cargo build --release;\
    rm -rf target/release/deps/$pkg* src

COPY . ./

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    set -eux; \
    cargo build --release; \
    objcopy --compress-debug-sections target/release/${pkg} ./${pkg}

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

WORKDIR /opt/${pkg}

COPY --from=builder /build/${pkg} ./${pkg}
COPY --from=web-builder --chown=${uid}:${gid} /build/dist /opt/${pkg}/web
RUN mkdir -p /opt/${pkg}/uploads && chown ${uid}:${gid} /opt/${pkg}/uploads
RUN mkdir -p /opt/${pkg}/tmp && chown ${uid}:${gid} /opt/${pkg}/tmp

ENV RUST_LOG=info
ENV ROCKET_CLI_COLOR="true"
ENV ROCKET_PORT="8080"
ENV ROCKET_ADDRESS="0.0.0.0"
ENV ROCKET_LIMITS='{file="5 MiB"}'
ENV ROCKET_TMP_DIR="/opt/${pkg}/tmp"
ENV FOLIO_WEB_PATH="/opt/${pkg}/web"
ENV FOLIO_UPLOADS_PATH="/opt/${pkg}/uploads"

EXPOSE 8080/tcp

ENTRYPOINT ["./${pkg}"]