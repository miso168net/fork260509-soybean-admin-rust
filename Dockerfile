# =============================================================================
# rust-api Dockerfile — rev1 deploy 版本(W-F1)
#
# 變動 vs 既有 alpine+musl 版本:
# - base image: rust:1.86-alpine + alpine:3.21 → rust:1.86-slim-bookworm + debian:bookworm-slim
# - openssl: static(musl-libs-static) → dynamic(libssl3)
# - 同時 build 兩個 binary:server + migration(支援 W-F8 migration init container)
# - runtime 加 curl(W-F3 compose healthcheck 用)
# - non-root user:appuser → rust-api(uid 10001 沿用)
# - 移除 --no-default-features(走 default features per brainstorm Q2)
# - 加 ENV APP_SERVER_PORT=11081(per Q1 clarify、走 F1.1 env-override / application.yaml 不動)
#
# 不在 W-F1 範疇:多 arch build / HEALTHCHECK directive / migration entrypoint
# / cleanup-job entrypoint / secret 注入 / :latest tag(留 W-F3 ~ W-F18)
# =============================================================================

ARG RUST_VERSION=1.86
ARG DEBIAN_CODENAME=bookworm
ARG APP_USER=rust-api
ARG APP_UID=10001
ARG APP_PORT=11081
ARG TZ=Asia/Shanghai

# -----------------------------------------------------------------------------
# Stage 1: builder
# -----------------------------------------------------------------------------
FROM rust:${RUST_VERSION}-slim-${DEBIAN_CODENAME} AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev ca-certificates git \
    && rm -rf /var/lib/apt/lists/*

# COPY 整個 workspace(.dockerignore 已排除 /target /deploy /.idea /.vscode 等)
COPY . .

# BuildKit cache mount 加速 incremental build(per FR-003)
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release --bin server --bin migration && \
    cp target/release/server /tmp/server && \
    cp target/release/migration /tmp/migration && \
    strip /tmp/server /tmp/migration

# -----------------------------------------------------------------------------
# Stage 2: runtime
# -----------------------------------------------------------------------------
FROM debian:${DEBIAN_CODENAME}-slim AS runtime

ARG APP_USER
ARG APP_UID
ARG APP_PORT
ARG TZ

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates libssl3 curl tzdata \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -g ${APP_UID} ${APP_USER} \
    && useradd -r -u ${APP_UID} -g ${APP_UID} -m -d /home/${APP_USER} -s /usr/sbin/nologin ${APP_USER}

WORKDIR /app

COPY --from=builder /tmp/server /usr/local/bin/server
COPY --from=builder /tmp/migration /usr/local/bin/migration
COPY --from=builder --chown=${APP_USER}:${APP_USER} /app/server/resources/application.yaml /app/server/resources/
COPY --from=builder --chown=${APP_USER}:${APP_USER} /app/server/resources/ip2region.xdb /app/server/resources/
COPY --from=builder --chown=${APP_USER}:${APP_USER} /app/server/resources/rbac_model.conf /app/server/resources/

USER ${APP_USER}
EXPOSE ${APP_PORT}

ENV TZ=${TZ} \
    LANG=en_US.UTF-8 \
    RUST_ENV=production \
    APP_SERVER_PORT=${APP_PORT}

ENTRYPOINT ["/usr/local/bin/server"]
