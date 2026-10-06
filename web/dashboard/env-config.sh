#!/bin/sh
# The nginx image's entrypoint runs this before starting nginx (the Dockerfile
# copies it into /docker-entrypoint.d/). It writes the container's VITE_*
# environment variables to env-config.js, which index.html loads before the
# app, so e.g. an ECS task can point the dashboard at its own API and Keycloak
# without a rebuild. Unset ones fall back to the values baked in at build time.
set -eu

out=/usr/share/nginx/html/env-config.js

{
  echo 'window.__ENV__ = {'
  for name in $(env | sed -n 's/^\(VITE_[A-Z0-9_]*\)=.*/\1/p' | sort); do
    # Escape backslashes and double quotes for a JS string literal.
    value=$(printenv "$name" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g')
    printf '  "%s": "%s",\n' "$name" "$value"
  done
  echo '};'
} > "$out"

echo "$0: wrote $out"
