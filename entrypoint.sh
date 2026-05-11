#!/bin/sh
# admin-api entrypoint：required env 預檢 + APP_JWT_ISSUER sentinel rejection + envsubst render + exec server
# per specs/007-dockerfile-envsubst/contracts/entrypoint-contract.md
set -eu

TEMPLATE=/app/server/resources/application.yaml.tpl
OUTPUT=/app/server/resources/application.yaml

log() { echo "[entrypoint] $*" >&2; }
fatal() { echo "[entrypoint] FATAL: $1" >&2; exit "${2:-1}"; }

log "starting..."

# 1. Required env 預檢（4 secret env，缺一即 exit 1）
log "validating required envs..."
for v in APP_DATABASE_URL APP_REDIS_URL APP_JWT_JWT_SECRET APP_JWT_ISSUER; do
  eval "value=\${${v}:-}"
  [ -n "${value}" ] || fatal "${v} is empty or unset" 1
done

# 2. APP_JWT_ISSUER sentinel rejection（exact-match list；per clarify Q1）
log "validating APP_JWT_ISSUER not placeholder..."
case "${APP_JWT_ISSUER}" in
  ""|"https://github.com/your-org/new-admin"|"change-me-issuer-url")
    fatal "APP_JWT_ISSUER='${APP_JWT_ISSUER}' is a known placeholder; set a real issuer URL" 2
    ;;
esac

# 3. envsubst render template → application.yaml
log "rendering application.yaml from template..."
envsubst < "${TEMPLATE}" > "${OUTPUT}" || fatal "envsubst rendering failed" 3
[ -s "${OUTPUT}" ] || fatal "rendered file is empty or write failed" 4

# 4. exec server（PID 1 換成 server，signal 正確傳遞）
log "exec $*"
exec "$@"
