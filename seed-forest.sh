#!/usr/bin/env bash
set -e
trap 'echo "❌ Error on line $LINENO. Command: $BASH_COMMAND" >&2' ERR

VERBOSE=false
if [ "$1" = "--verbose" ]; then
    VERBOSE=true
fi

log() {
    if [ "$VERBOSE" = true ]; then
        echo "$@"
    fi
}

get_workspace_version() {
    local crate_name=$1
    grep '^version = ' "crates/${crate_name}/Cargo.toml" | head -1 | cut -d'"' -f2
}

get_installed_version() {
    local binary=$1
    if command -v "$binary" &>/dev/null; then
        "$binary" --version 2>/dev/null | awk '{print $2}'
    else
        echo ""
    fi
}

install_if_needed() {
    local binary=$1
    local crate_name=$2
    local workspace_version=$(get_workspace_version "$crate_name")
    local installed_version=$(get_installed_version "$binary")

    if [ "$installed_version" = "$workspace_version" ]; then
        log "✓ $binary $workspace_version already installed"
        return 0
    fi

    if [ -n "$installed_version" ]; then
        log "Updating $binary from $installed_version to $workspace_version..."
    else
        log "Installing $binary $workspace_version..."
    fi

    cargo install --path "crates/${crate_name}" &>/dev/null
    log "✓ $binary installed"
}

# Install forest tools first
log "Checking forest tools..."
install_if_needed "sembly" "sembly-cli"
install_if_needed "forester" "forester"
install_if_needed "brdev" "brdev"

# Use forester to seed the forest
if [ "$VERBOSE" = true ]; then
    forester seed --verbose
else
    forester seed
fi

# Build sembly index
log "Building sembly index..."
sembly build 2>/dev/null

# Verify
if ! sembly search "test" 2>/dev/null | head -1 | grep -q "Found"; then
    echo "❌ Setup verification failed" >&2
    exit 1
fi

log "✅ Setup complete"
