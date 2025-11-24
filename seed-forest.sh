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

# Build forester
if [ ! -f ./forester/target/release/forester ]; then
    log "Building forester..."
    cd forester
    cargo build --release &>/dev/null
    cd ..
    log "✓ forester built"
else
    log "✓ forester already built"
fi

# Build knowledge index
if ! ./forester/target/release/forester index status &>/dev/null; then
    log "Building knowledge index..."
    ./forester/target/release/forester index build &>/dev/null
    log "✓ Knowledge index built"
else
    log "✓ Knowledge index already exists"
fi

# Verify
if ! ./forester/target/release/forester index search "test" 2>/dev/null | head -1 | grep -q "Found"; then
    echo "❌ Setup verification failed" >&2
    exit 1
fi

log "✅ Setup complete"
