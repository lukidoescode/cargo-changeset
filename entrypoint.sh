#!/bin/sh
set -e
git config --global --add safe.directory "$(pwd)"
exec cargo-changeset "$@"
