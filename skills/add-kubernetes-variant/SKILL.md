---
name: add-kubernetes-variant
description: Add a new Kubernetes variant to the bottlerocket repository
---

# Add Kubernetes Variant

Add a new Kubernetes variant to the bottlerocket repository that consumes packages from core-kit.

## Purpose

After packaging Kubernetes in core-kit (via `add-kubernetes-prerelease`), this skill creates the variant definition in the bottlerocket repo that uses those packages.

## When to Use

- After `add-kubernetes-prerelease` has created kubernetes-{version} packages in core-kit
- When adding support for a new Kubernetes minor version
- When creating platform variants (aws, vmware) for a new k8s version

## Prerequisites

1. Core-kit with kubernetes-{version} packages published to registry
2. Worktree created for the k8s version: `forester worktree create k8s-{version}`
3. Reference variant exists (e.g., aws-k8s-1.34 when creating aws-k8s-1.35)

## Variables

Set these before starting:

```bash
PREV_VERSION="1.34"      # Previous k8s version to copy from
NEW_VERSION="1.35"       # New k8s version
PLATFORM="aws"           # Platform: aws, vmware, metal
WORKTREE="k8s-1.35"      # Worktree name
```

## Procedure

### Step 1: Create Settings-Defaults Directory

```bash
cd worktrees/${WORKTREE}/bottlerocket

# Copy from previous version
cp -r sources/settings-defaults/${PLATFORM}-k8s-${PREV_VERSION}       sources/settings-defaults/${PLATFORM}-k8s-${NEW_VERSION}

# Update Cargo.toml package name (replace dots with underscores in version)
PREV_UNDERSCORE=$(echo $PREV_VERSION | tr '.' '_')
NEW_UNDERSCORE=$(echo $NEW_VERSION | tr '.' '_')

sed -i "s/${PLATFORM}-k8s-${PREV_UNDERSCORE}/${PLATFORM}-k8s-${NEW_UNDERSCORE}/g"     sources/settings-defaults/${PLATFORM}-k8s-${NEW_VERSION}/Cargo.toml
```

The defaults.d/ symlinks are inherited and don't need modification.

### Step 2: Create Variant Directory

```bash
# Copy variant directory
cp -r variants/${PLATFORM}-k8s-${PREV_VERSION}       variants/${PLATFORM}-k8s-${NEW_VERSION}

# Update Cargo.toml
sed -i "s/${PLATFORM}-k8s-${PREV_UNDERSCORE}/${PLATFORM}-k8s-${NEW_UNDERSCORE}/g"     variants/${PLATFORM}-k8s-${NEW_VERSION}/Cargo.toml

# Update kubelet version reference
sed -i "s/kubelet-${PREV_VERSION}/kubelet-${NEW_VERSION}/g"     variants/${PLATFORM}-k8s-${NEW_VERSION}/Cargo.toml
```

### Step 3: Update sources/Cargo.toml

Add the new settings-defaults to workspace members:

```bash
# Find the line with previous version and add new one after it
sed -i "/settings-defaults\/${PLATFORM}-k8s-${PREV_VERSION}"/a\    \"settings-defaults/${PLATFORM}-k8s-${NEW_VERSION}\","     sources/Cargo.toml
```

### Step 4: Update Root Cargo.toml

Add the new variant to workspace members:

```bash
# Add after previous version entry
sed -i "/variants\/${PLATFORM}-k8s-${PREV_VERSION}",/a\    \"variants/${PLATFORM}-k8s-${NEW_VERSION}\","     Cargo.toml
```

### Step 5: Update settings-defaults.spec

Edit `packages/settings-defaults/settings-defaults.spec`:

**Add package definition block** (after the ${PREV_VERSION} block):

```spec
%package ${PLATFORM}-k8s-${NEW_VERSION}
Summary: Settings defaults for the ${PLATFORM}-k8s ${NEW_VERSION} variants
Requires: (%{shrink:
           %{_cross_os}variant(${PLATFORM}-k8s-${NEW_VERSION})      or
           %{_cross_os}variant(${PLATFORM}-k8s-${NEW_VERSION}-fips)
           %{nil}})
Provides: %{_cross_os}settings-defaults(any)
Provides: %{_cross_os}settings-defaults(${PLATFORM}-k8s-${NEW_VERSION})
Provides: %{_cross_os}settings-defaults(${PLATFORM}-k8s-${NEW_VERSION}-fips)
Conflicts: %{_cross_os}settings-defaults(any)

%description ${PLATFORM}-k8s-${NEW_VERSION}
%{summary}.
```

**Add %files section** (after the ${PREV_VERSION} %files block):

```spec
%files ${PLATFORM}-k8s-${NEW_VERSION}
%{_cross_defaultsdir}/${PLATFORM}-k8s-${NEW_VERSION}.toml
%{_cross_tmpfilesdir}/storewolf-defaults-${PLATFORM}-k8s-${NEW_VERSION}.conf
```

### Step 6: Update settings-plugins.spec

Edit `packages/settings-plugins/settings-plugins.spec`:

**Add Provides lines** to the appropriate package section (find the ${PLATFORM}-k8s package):

```spec
Provides: %{_cross_os}settings-plugin(${PLATFORM}-k8s-${NEW_VERSION})
Provides: %{_cross_os}settings-plugin(${PLATFORM}-k8s-${NEW_VERSION}-fips)
```

### Step 7: Update README.md

Add the new variant to the variants list in README.md.

### Step 8: Regenerate Cargo.lock

```bash
cargo generate-lockfile
cd sources && cargo generate-lockfile && cd ..
```

### Step 9: Commit Changes

```bash
git add -A
git commit -m "variants: add ${PLATFORM}-k8s-${NEW_VERSION}"
```

## Validation

1. Verify directories exist:
   ```bash
   ls variants/${PLATFORM}-k8s-${NEW_VERSION}/
   ls sources/settings-defaults/${PLATFORM}-k8s-${NEW_VERSION}/
   ```

2. Verify Cargo.toml entries:
   ```bash
   grep "${PLATFORM}-k8s-${NEW_VERSION}" Cargo.toml
   grep "${PLATFORM}-k8s-${NEW_VERSION}" sources/Cargo.toml
   ```

3. Build the variant (requires local registry with core-kit):
   ```bash
   cargo make -e BUILDSYS_VARIANT=${PLATFORM}-k8s-${NEW_VERSION} build-variant
   ```

## Common Issues

### "package not found" during build
- Ensure core-kit with kubernetes-${NEW_VERSION} is published to local registry
- Check Twoliter.toml points to correct registry

### Symlink errors
- The defaults.d/ symlinks are relative paths - don't modify them
- They should all point to ../../../shared-defaults/

### Cargo.lock conflicts
- Run `cargo generate-lockfile` in both root and sources/ directories

## Notes

- FIPS variants share settings-defaults with non-FIPS (no separate directory needed)
- nvidia variants need separate settings-defaults (different containerd config)
- For nvidia variants, also copy from ${PLATFORM}-k8s-${PREV_VERSION}-nvidia
