# syntax=docker/dockerfile:1

# The build context is the repo root, so the build can reach the TypeScript
# SDK in clients/typescript that the demo store at /demo runs on:
#   docker build -f web/dashboard/Dockerfile .
# Dockerfile.dockerignore, next to this file, keeps the context to those two.
#
# VITE_* settings can be set in two places:
#   - At run time, as the container's environment variables
#     (docker run -e VITE_KEYCLOAK_URL=https://..., or an ECS task's
#     `environment`). env-config.sh writes them out when the container starts.
#   - At build time, as build args (--build-arg VITE_KEYCLOAK_URL=https://...),
#     which Vite bakes into the bundle.
# Run-time values win. Any left unset or empty in both fall back to the
# defaults in src/config/api.ts.

FROM node:24-bookworm-slim AS builder

# Mirror the repo layout, so vite.config.ts finds the SDK at
# ../../clients/typescript.
WORKDIR /app/web/dashboard

COPY web/dashboard/package.json web/dashboard/package-lock.json ./
RUN npm ci

COPY clients/typescript /app/clients/typescript
COPY web/dashboard ./

ARG VITE_API_BASE_URL
ARG VITE_KEYCLOAK_URL
ARG VITE_KEYCLOAK_REALM
ARG VITE_KEYCLOAK_CLIENT_ID
ARG VITE_KEYCLOAK_REDIRECT_URI
ARG VITE_KEYCLOAK_POST_LOGOUT_URI
ARG VITE_KEYCLOAK_IDP_ALIAS
ARG VITE_ANALYTICS_TOKEN
ARG VITE_ANALYTICS_FLUSH_INTERVAL
ARG VITE_MAINTENANCE_MODE
ARG VITE_DEMO_ENABLED
ARG VITE_DEMO_COLLECTOR_URL
ARG VITE_DEMO_TENANT_ID
ARG VITE_DEMO_WORKSPACE_ID

# An empty VITE_* would be baked in as "" and win over the app's `??`
# defaults, so drop the empty ones before building.
RUN for var in $(env | sed -n 's/^\(VITE_[A-Z_]*\)=$/\1/p'); do unset "$var"; done \
    && npm run build

FROM nginx:stable-alpine AS runtime

COPY web/dashboard/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/web/dashboard/dist /usr/share/nginx/html
COPY --chmod=755 web/dashboard/env-config.sh /docker-entrypoint.d/40-env-config.sh

EXPOSE 80
