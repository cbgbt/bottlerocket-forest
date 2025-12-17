---
name: update-package
description: Update an existing package to a new upstream version
---

# Update Package

Update an existing Bottlerocket package to a new upstream version.

## When to Use

- Upstream releases a new version of a packaged component
- Security fix requires version bump
- New features needed from upstream

## Prerequisites

- Package already exists in a kit
- New upstream version available
- Understanding of package structure (Cargo.toml, spec file)

## Procedure

### 1. Identify Package Location

```bash
# Find the package
find kits/ -type d -name "PACKAGE_NAME"

# Common locations:
# kits/bottlerocket-core-kit/packages/
# kits/bottlerocket-kernel-kit/packages/
```

### 2. Check Current Version

```bash
cd kits/<kit-name>/packages/<package-name>

# View current version in Cargo.toml
grep -A5 "external-files" Cargo.toml

# View current version in spec
grep "^Version:" *.spec
```

### 3. Download New Source

```bash
# Download from upstream
curl -LO <new-source-url>

# If signature exists
curl -LO <new-source-url>.sig
```

### 4. Calculate Checksums

```bash
sha512sum <new-source-file>
sha512sum <new-source-file>.sig  # if applicable
```

### 5. Verify GPG Signature (if applicable)

**For GPG-signed packages only** (check if package directory has `gpgkey-*.asc` file):

```bash
# Import the package's GPG key
gpg --import /path/to/worktree/kits/<kit-name>/packages/<package>/gpgkey-*.asc

# Verify the signature
gpg --verify <new-source-file>.sig <new-source-file>
# or for .asc files:
gpg --verify <new-source-file>.asc <new-source-file>
```

**Expected output:**
```
gpg: Signature made ...
gpg: Good signature from "Upstream Maintainer <email>"
```

**If verification fails:**
- Check if the signing key has changed (see upstream release notes)
- Download the new public key from upstream's trusted source
- Replace `gpgkey-*.asc` in the package directory
- Re-import and verify again

**For unsigned packages:**
- Skip this step if no `gpgkey-*.asc` file exists in the package directory
- Some packages (like containerd, amazon-ecs-cni-plugins) don't use GPG signatures
- Proceed directly to updating Cargo.toml

### 6. Update Cargo.toml

Edit `[[package.metadata.build-package.external-files]]` sections:

```toml
[[package.metadata.build-package.external-files]]
path = "package-NEW_VERSION.tar.gz"
url = "https://example.com/package-NEW_VERSION.tar.gz"
sha512 = "<new-checksum>"

# If signature file exists:
[[package.metadata.build-package.external-files]]
path = "package-NEW_VERSION.tar.gz.sig"
url = "https://example.com/package-NEW_VERSION.tar.gz.sig"
sha512 = "<new-signature-checksum>"
```

### 7. Update .spec File

The format varies by package type:

**Standard package:**
```spec
Name: %{_cross_os}package-name
Version: NEW_VERSION
Release: 1%{?dist}
```

**Kubernetes-versioned package:**
```spec
%global kubever 1.32
%global k8s_minor 32
Version: NEW_VERSION
```

**Go package:**
```spec
%global goproject github.com/org
%global gorepo repo
%global goimport %{goproject}/%{gorepo}
Version: NEW_VERSION
Source0: https://example.com/package-%{version}.tar.gz
```

### 8. Test Build

Build and test the single package:

```bash
# From the kit directory (e.g., kits/bottlerocket-core-kit/)
PACKAGE=<package-name> make twoliter build-package -e BUILDSYS_UPSTREAM_SOURCE_FALLBACK=true
```

This builds only the specified package, fetching sources directly from upstream URLs since they're not in the lookaside cache yet.

**For batch updates:**
- When updating multiple related packages in one PR, validate each with single-package builds (can run in parallel)
- After all packages pass individual builds, do one full kit build using the `build-kit-locally` skill
- Single-package builds are faster for iteration; full kit build provides final validation
### 9. Handle Build Failures

**Patch application failures:**
- Check if patches are still needed (upstream may have incorporated the fix)
- Update patch files for new source structure
- Remove obsolete patches from spec file

**Dependency version conflicts:**
- Check if bundled dependencies changed versions
- Update `Provides: bundled()` in spec file

**New build dependencies:**
- Update `[build-dependencies]` in Cargo.toml
- Update `BuildRequires` in .spec file

### 10. Create Commit

After successful build, create a signed commit:

```bash
git add -A
git commit -s -m "package-name: update to NEW_VERSION"
```

## Validation

- [ ] New source downloads successfully
- [ ] Checksums match downloaded files
- [ ] GPG signature verifies (if applicable)
- [ ] Package builds without errors
- [ ] Version appears correctly in build output

## Common Issues

**404 on source URL:** Check upstream for correct download location.

**Patch conflicts:** Review patches against new source, update or remove as needed.

**Missing dependencies:** Check upstream release notes for new requirements.

## Reference

- Package Cargo.toml for current configuration
- Upstream release notes for changes
- Kit's README for build instructions
