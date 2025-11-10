#!/usr/bin/env bash
set -e

REPOS=(
    "bottlerocket"
    "bottlerocket-core-kit"
    "bottlerocket-kernel-kit"
    "twoliter"
    "bottlerocket-sdk"
    "bottlerocket-admin-container"
    "bottlerocket-control-container"
    "bottlerocket-settings-sdk"
)

for repo in "${REPOS[@]}"; do
    if [ -d "$repo" ]; then
        echo "✓ $repo already exists"
    else
        echo "Cloning $repo..."
        git clone "git@github.com:bottlerocket-os/${repo}.git"
    fi
done

echo "Forest seeded successfully!"
