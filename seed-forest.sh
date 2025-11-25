#!/usr/bin/env bash
set -e

VERBOSE=false
if [ "$1" = "--verbose" ]; then
    VERBOSE=true
fi

log() {
    if [ "$VERBOSE" = true ]; then
        echo "$@"
    fi
}

clone_if_missing() {
    local dir=$1
    local repo=$2
    
    if [ -d "$dir" ]; then
        log "✓ $dir already exists"
    else
        log "Cloning $repo into $dir..."
        git clone "git@github.com:bottlerocket-os/${repo}.git" "$dir" &>/dev/null
        log "✓ Cloned $dir"
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

# Clone repositories
log "Cloning repositories..."
clone_if_missing "bottlerocket" "bottlerocket"
clone_if_missing "kits/bottlerocket-core-kit" "bottlerocket-core-kit"
clone_if_missing "kits/bottlerocket-kernel-kit" "bottlerocket-kernel-kit"
clone_if_missing "twoliter" "twoliter"
clone_if_missing "sdk/bottlerocket-sdk" "bottlerocket-sdk"
clone_if_missing "host-containers/bottlerocket-admin-container" "bottlerocket-admin-container"
clone_if_missing "host-containers/bottlerocket-control-container" "bottlerocket-control-container"
clone_if_missing "bottlerocket-settings-sdk" "bottlerocket-settings-sdk"

# Install forest tools
log "Checking forest tools..."
install_if_needed "sembly" "sembly-cli"
install_if_needed "forester" "forester"

# Build knowledge index
if ! sembly status &>/dev/null; then
    log "Building knowledge index..."
    sembly build &>/dev/null
    log "✓ Knowledge index built"
else
    log "✓ Knowledge index already exists"
fi

# Verify
if ! sembly search "test" 2>/dev/null | head -1 | grep -q "Found"; then
    echo "❌ Setup verification failed" >&2
    exit 1
fi

log "✅ Setup complete"
