database:
    url: "${APP_DATABASE_URL}"
    max_connections: ${APP_DATABASE_MAX_CONNECTIONS}
    min_connections: 1
    connect_timeout: 30
    idle_timeout: 600
server:
    host: "${APP_SERVER_HOST}"
    port: ${APP_SERVER_PORT}
jwt:
    jwt_secret: "${APP_JWT_JWT_SECRET}"
    issuer: "${APP_JWT_ISSUER}"
    expire: ${APP_JWT_EXPIRE}
redis:
    mode: single
    url: "${APP_REDIS_URL}"
