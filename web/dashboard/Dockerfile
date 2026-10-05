# syntax=docker/dockerfile:1

# The build context is this directory:
#   docker build -f web/dashboard/Dockerfile web/dashboard
#
# Vite bakes VITE_* values into the bundle at build time, so pass them as
# build args (e.g. --build-arg VITE_KEYCLOAK_URL=https://...). Any left unset
# or empty fall back to the defaults in src/config/api.ts.

FROM node:24-bookworm-slim AS builder

WORKDIR /app

COPY package.json package-lock.json ./
RUN npm ci

COPY . .

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

# An empty VITE_* would be baked in as "" and win over the app's `??`
# defaults, so drop the empty ones before building.
RUN for var in $(env | sed -n 's/^\(VITE_[A-Z_]*\)=$/\1/p'); do unset "$var"; done \
    && npm run build

FROM nginx:stable-alpine AS runtime

COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/dist /usr/share/nginx/html

EXPOSE 80
