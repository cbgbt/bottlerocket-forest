#!/usr/bin/env bash
set -e

clone_if_missing() {
    local dir=$1
    local repo=$2
    
    if [ -d "$dir" ]; then
        echo "✓ $dir already exists"
    else
        echo "Cloning $repo into $dir..."
        git clone "git@github.com:bottlerocket-os/${repo}.git" "$dir"
    fi
}

clone_if_missing "bottlerocket" "bottlerocket"
clone_if_missing "kits/bottlerocket-core-kit" "bottlerocket-core-kit"
clone_if_missing "kits/bottlerocket-kernel-kit" "bottlerocket-kernel-kit"
clone_if_missing "twoliter" "twoliter"
clone_if_missing "sdk/bottlerocket-sdk" "bottlerocket-sdk"
clone_if_missing "host-containers/bottlerocket-admin-container" "bottlerocket-admin-container"
clone_if_missing "host-containers/bottlerocket-control-container" "bottlerocket-control-container"
clone_if_missing "bottlerocket-settings-sdk" "bottlerocket-settings-sdk"

echo "Forest seeded successfully!"
