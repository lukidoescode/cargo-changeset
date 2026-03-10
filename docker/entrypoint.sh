#!/bin/sh
set -e
git config --global --add safe.directory "$(pwd)"

if [ -n "${GIT_FETCH_CHANGESET_BASE}" ]; then
    git fetch "${GIT_FETCH_REMOTE:-origin}" "${GIT_FETCH_CHANGESET_BASE}" || true
fi

exec cargo-changeset "$@"
