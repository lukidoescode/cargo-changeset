#!/bin/sh
set -e

# If already running as non-root (e.g., via --user flag), skip privilege management
if [ "$(id -u)" != "0" ]; then
    git config --global --add safe.directory "$(pwd)"
    exec cargo-changeset "$@"
fi

# Default to the built-in non-root user
TARGET_UID=1000
TARGET_GID=1000

# Detect owner UID/GID of the working directory
WORK_DIR="$(pwd)"
if [ -d "$WORK_DIR" ] && [ "$WORK_DIR" != "/" ]; then
    TARGET_UID=$(stat -c '%u' "$WORK_DIR")
    TARGET_GID=$(stat -c '%g' "$WORK_DIR")
fi

# If workspace is owned by root, fall back to default non-root user
if [ "$TARGET_UID" = "0" ]; then
    TARGET_UID=1000
    TARGET_GID=1000
fi

# Ensure writable home directory for git config
USER_HOME="/home/changeset"
mkdir -p "$USER_HOME"
chown "$TARGET_UID:$TARGET_GID" "$USER_HOME"

# Configure git safe.directory
su-exec "$TARGET_UID:$TARGET_GID" env HOME="$USER_HOME" git config --global --add safe.directory "$WORK_DIR"

# Run as the target user
exec su-exec "$TARGET_UID:$TARGET_GID" env HOME="$USER_HOME" cargo-changeset "$@"
