#!/usr/bin/env bash
# Compute the next semantic version for a service from conventional commit
# messages (https://www.conventionalcommits.org) since the latest
# `<service>-vX.Y.Z` git tag.
#
#   <type>!... or BREAKING CHANGE: footer   -> major bump
#   feat:                                  -> minor bump
#   fix:                                   -> patch bump
#   anything else                          -> no release (no output)
#
# With no previous tag the base is 0.0.0, so the first feat ships 0.1.0.
# Prints the bare version (e.g. `1.4.2`) to stdout when a release is due.
#
# Usage: next-version.sh [service]   (default: event-collector)

set -euo pipefail

service="${1:-event-collector}"
prefix="${service}-v"

latest_tag="$(git tag --list "${prefix}*" --sort=-v:refname | head -n 1 || true)"

if [ -n "$latest_tag" ]; then
    range="${latest_tag}..HEAD"
    current="${latest_tag#"${prefix}"}"
else
    range="HEAD"
    current="0.0.0"
fi

messages="$(git log "$range" --pretty=format:%B)"

bump=""
if grep -qE '^[a-zA-Z]+(\([^)]*\))?!:' <<<"$messages" ||
    grep -qE '^BREAKING CHANGE:' <<<"$messages"; then
    bump="major"
elif grep -qE '^feat(\([^)]*\))?!?:' <<<"$messages"; then
    bump="minor"
elif grep -qE '^fix(\([^)]*\))?!?:' <<<"$messages"; then
    bump="patch"
fi

if [ -z "$bump" ]; then
    exit 0
fi

IFS='.' read -r major minor patch <<<"$current"

case "$bump" in
    major) major=$((major + 1)); minor=0; patch=0 ;;
    minor) minor=$((minor + 1)); patch=0 ;;
    patch) patch=$((patch + 1)) ;;
esac

echo "${major}.${minor}.${patch}"
