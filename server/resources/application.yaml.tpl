database:
    url: "${APP_DATABASE_URL}"
    max_connections: ${APP_DATABASE_MAX_CONNECTIONS:-10}
    min_connections: 1
    connect_timeout: 30
    idle_timeout: 600
server:
    host: "${APP_SERVER_HOST:-0.0.0.0}"
    port: ${APP_SERVER_PORT:-10001}
jwt:
    jwt_secret: "${APP_JWT_JWT_SECRET}"
    issuer: "${APP_JWT_ISSUER}"
    expire: ${APP_JWT_EXPIRE:-7200}
redis:
    mode: single
    url: "${APP_REDIS_URL}"
