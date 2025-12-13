---
name: add-kubernetes-prerelease
description: Package Kubernetes pre-release (beta/rc) sources from EKS-D before GA
---

# Add Kubernetes Pre-Release

Package Kubernetes beta/rc versions from EKS-D pre-release artifacts.

## Purpose

Creates ecr-credential-provider and kubernetes packages for pre-release versions (beta, rc) using EKS-D pre-release artifacts.
Differs from production packaging by using pre-release URLs and version strings with suffixes.

## When to Use

- Testing new Kubernetes versions before GA release
- EKS-D pre-release artifacts are available
- Need to validate breaking changes early

## Prerequisites

- Run `analyze-kubernetes-release` skill first
- EKS-D pre-release artifacts published at https://distro.eks.amazonaws.com/kubernetes-{major}-{minor}/releases/
- Working in bottlerocket-core-kit repository (or forest worktree)
- Previous Kubernetes version packages exist to copy from

## Tracking

Create a tracking issue following this pattern:
https://github.com/bottlerocket-os/bottlerocket/issues/4620

Checklist items:
- [ ] Use pre-release sources in packages (this skill)
- [ ] Use official GA sources in packages (promote-kubernetes-ga)
- [ ] Add variants for k8s version (bottlerocket repo)

## Procedure

### 1. Set Version Variables

```bash
# Set versions (example: adding 1.NN based on 1.MM)
PREV_VERSION="1.MM"      # e.g., "1.34"
NEW_VERSION="1.NN"       # e.g., "1.35"
PREV_VER_NODOT="1MM"     # e.g., "134" (used in package names)
NEW_VER_NODOT="1NN"      # e.g., "135"
PRERELEASE_TAG="beta.0"  # or "rc.1", etc.
FULL_VERSION="${NEW_VERSION}.0-${PRERELEASE_TAG}"

# Verify analysis exists
ANALYSIS="$FOREST_ROOT/planning/k8s-${NEW_VERSION}-analysis.md"
if [ ! -f "$ANALYSIS" ]; then
    echo "ERROR: Run analyze-kubernetes-release first"
    exit 1
fi

cd kits/bottlerocket-core-kit/packages
```

### 2. Verify Pre-release Sources Exist

**Critical:** Verify EKS-D pre-release artifacts are published before making any changes.

```bash
# Construct pre-release URLs
RELEASE="1"  # EKS-D release number, typically "1" for first pre-release
K8S_URL="https://distro.eks.amazonaws.com/kubernetes-${NEW_VERSION}/releases/${RELEASE}/artifacts/kubernetes/v${FULL_VERSION}/kubernetes-src.tar.gz"
ECR_URL="https://distro.eks.amazonaws.com/kubernetes-${NEW_VERSION}/releases/${RELEASE}/artifacts/plugins/v${FULL_VERSION}/eks-distro-ecr-credential-provider-linux-amd64-v${FULL_VERSION}.tar.gz"

# Verify kubernetes source exists
if ! curl -sfI "$K8S_URL" > /dev/null; then
    echo "ERROR: Kubernetes source not found at $K8S_URL"
    echo "Check https://distro.eks.amazonaws.com/kubernetes-${NEW_VERSION}/releases/ for available releases"
    exit 1
fi

# Verify ecr-credential-provider source exists
if ! curl -sfI "$ECR_URL" > /dev/null; then
    echo "ERROR: ECR credential provider not found at $ECR_URL"
    echo "Check https://distro.eks.amazonaws.com/kubernetes-${NEW_VERSION}/releases/ for available releases"
    exit 1
fi

echo "✓ Pre-release sources verified"
```

### 2. Copy ecr-credential-provider Package

```bash
cp -r ecr-credential-provider-${PREV_VER_NODOT} ecr-credential-provider-${NEW_VER_NODOT}
cd ecr-credential-provider-${NEW_VER_NODOT}

# Update Cargo.toml
sed -i "s/ecr-credential-provider-${PREV_VER_NODOT}/ecr-credential-provider-${NEW_VER_NODOT}/g" Cargo.toml
sed -i "s/${PREV_VERSION}/${NEW_VERSION}/g" Cargo.toml

# Update spec file
mv ecr-credential-provider-${PREV_VER_NODOT}.spec ecr-credential-provider-${NEW_VER_NODOT}.spec
sed -i "s/${PREV_VERSION}/${NEW_VERSION}/g" ecr-credential-provider-${NEW_VER_NODOT}.spec

cd ..
```

### 4. Copy kubernetes Package

```bash
cp -r kubernetes-${PREV_VER_NODOT} kubernetes-${NEW_VER_NODOT}
cd kubernetes-${NEW_VER_NODOT}

# Update Cargo.toml
sed -i "s/kubernetes-${PREV_VER_NODOT}/kubernetes-${NEW_VER_NODOT}/g" Cargo.toml
sed -i "s/${PREV_VERSION}/${NEW_VERSION}/g" Cargo.toml

# Update spec file
mv kubernetes-${PREV_VER_NODOT}.spec kubernetes-${NEW_VER_NODOT}.spec
sed -i "s/${PREV_VERSION}/${NEW_VERSION}/g" kubernetes-${NEW_VER_NODOT}.spec
sed -i "s/${PREV_VER_NODOT}/${NEW_VER_NODOT}/g" kubernetes-${NEW_VER_NODOT}.spec

# Update template and systemd files
find . -type f \( -name "*.service" -o -name "*.conf" -o -name "*.toml" -o -name "*.env" \) \
    -exec sed -i "s/${PREV_VERSION}/${NEW_VERSION}/g" {} \;

cd ../..
```

### 5. Apply Breaking Changes from Analysis

**Critical:** Review the KEP analysis document and apply necessary changes.

Common changes between Kubernetes versions:
- Removed kubelet flags (check kubelet-exec-start-conf)
- New kubelet config options (check kubelet-config)
- Deprecated features requiring opt-in/opt-out

Refer to `$FOREST_ROOT/planning/k8s-${NEW_VERSION}-analysis.md` for version-specific changes.

### 6. Update Source URLs and Release Field

**Critical:** Pre-release URLs differ from production URLs.

Format: `https://distro.eks.amazonaws.com/kubernetes-{major}-{minor}/releases/{release}/artifacts/kubernetes/v{version}/...`

**Version Variables for Pre-release:**

The pre-release status is indicated in the Release field, NOT in Version.

**Example spec file structure:**

```spec
# Version variables - use clean version, pre-release in Release field only
%global gover 1.35.0
%global rpmver %{gover}

Name: %{_cross_os}kubernetes
Version: %{rpmver}
Release: 0.beta0%{?dist}

# Hardcode Source0 with actual pre-release version (will update for GA)
Source0: https://distro.eks.amazonaws.com/kubernetes-1-35/releases/1/artifacts/kubernetes/v1.35.0-beta.0/kubernetes-src.tar.gz
```

⚠️ **Why this matters:**
- Version field stays clean (1.35.0) for RPM compatibility
- Release field (0.beta0) sorts before GA release (1)
- Source0 URL contains the actual pre-release tag from upstream

```bash
# Update kubernetes spec
vim packages/kubernetes-${NEW_VER_NODOT}/kubernetes-${NEW_VER_NODOT}.spec
# Set: %global gover ${NEW_VERSION}.0
# Set: %global rpmver %{gover}
# Set: Release: 0.beta0%{?dist}
# Hardcode Source0 with full pre-release URL

# Update ecr-credential-provider Cargo.toml URL
vim packages/ecr-credential-provider-${NEW_VER_NODOT}/Cargo.toml
# Update the url field to point to the new version
```

**Critical: Set Pre-release Release Field**

For BOTH spec files, update the Release field to indicate pre-release:
```spec
Release: 0.beta0%{?dist}
```

Or for release candidates:
```spec
Release: 0.rc0%{?dist}
```

⚠️ **Why this matters:** RPM version ordering uses Release when Version is equal.
- `0.beta0` sorts before `1` (GA release)
- This ensures clean upgrades: pre-release → GA works correctly
- Using `Release: 1%{?dist}` for pre-release breaks upgrade ordering

The pattern is: `0.<prerelease-tag>%{?dist}` where prerelease-tag matches the source (beta0, rc0, rc1, etc.)

**Check for Obsolete Patches**

Review patches from the previous version:
```bash
# Check if patches still apply or were merged upstream
ls packages/kubernetes-${PREV_VER_NODOT}/*.patch 2>/dev/null
```

Remove patches that were merged upstream in the new release.

### 7. Update Workspace Configuration

```bash
cd ../..  # Back to core-kit root

# Edit Cargo.toml to add new packages to workspace members
vim Cargo.toml

# Add to [workspace] members array:
#   "packages/ecr-credential-provider-${NEW_VER_NODOT}",
#   "packages/kubernetes-${NEW_VER_NODOT}",
```

### 8. Build and Validate

**Required:** Run the `build-kit-locally` skill to verify packages compile correctly.

This builds the kit and publishes to the local registry, confirming the new packages work.

### 9. Commit Changes

Create separate commits for each package (following existing PR patterns):

```bash
# Commit 1: ecr-credential-provider
git add packages/ecr-credential-provider-${NEW_VER_NODOT}/
# Also add Cargo.toml with just the ecr-credential-provider line
git commit -m "packages: add ecr-credential-provider-${NEW_VER_NODOT}"

# Commit 2: kubernetes
git add packages/kubernetes-${NEW_VER_NODOT}/ Cargo.toml Cargo.lock
git commit -m "packages: add kubernetes-${NEW_VER_NODOT}"
```

## Validation

Success indicators:
- `build-kit-locally` skill completes successfully
- Kit visible in local registry via `brdev registry list`

## Common Issues

**404 on pre-release URLs:**
- EKS-D pre-release not published yet
- Check https://distro.eks.amazonaws.com/kubernetes-{major}-{minor}/releases/ for available releases
- Verify release number and pre-release tag in URL

**Version mismatch errors:**
- Ensure all URLs use full pre-release version (e.g., v1.NN.0-beta.0, not v1.NN)
- Check spec file Version field matches

**Build failures from removed flags:**
- Check KEP analysis for removed kubelet flags
- Update kubelet-exec-start-conf to remove deprecated flags

**Wrong Release field for pre-release:**
- Symptom: RPM version conflicts when promoting to GA
- Cause: Using `Release: 1%{?dist}` instead of `Release: 0.beta0%{?dist}`
- Fix: Pre-release packages must use `Release: 0.<tag>%{?dist}` to sort before GA

## Next Steps

After successful build and commit:
1. **Optional:** Build variant to test end-to-end (`build-variant-from-local-kits` skill)
2. Test thoroughly - pre-release versions may have bugs
3. When GA releases, promote to production (`promote-kubernetes-ga` skill)

## Related Skills

This skill is part of the Kubernetes version workflow:
1. `analyze-kubernetes-release` - Deep analysis of KEPs and changes (run first)
2. `add-kubernetes-prerelease` - Package beta/rc for early testing (this skill)
3. `build-kit-locally` - Build and publish kit (included in step 7)
4. `promote-kubernetes-ga` - Promote to GA (run after upstream GA)
