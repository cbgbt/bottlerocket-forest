---
name: promote-kubernetes-ga
description: Promote a pre-release Kubernetes version to GA by updating source URLs and removing pre-release markers
---

# Promote Kubernetes to GA

Promote a pre-release Kubernetes version to GA release in bottlerocket-core-kit.

## When to Use

- Kubernetes version moves from pre-release (alpha/beta/rc) to GA
- EKS-D releases the GA version
- Pre-release packages already exist in core-kit

## Prerequisites

- Pre-release packages exist (kubernetes-X.Y, ecr-credential-provider-X.Y)
- GA release available from upstream
- EKS-D has released the GA version

## Procedure

### 1. Identify Version

```bash
K8S_VERSION="1.32"
VER_NODOT="${K8S_VERSION//.}"  # "132"
```

### 2. Navigate to Packages

```bash
cd kits/bottlerocket-core-kit/packages
```

### 3. Update kubernetes Package

```bash
cd kubernetes-${VER_NODOT}
```

**Update Cargo.toml:**
- Change source URLs from pre-release to GA
- Update checksums for new sources
- Remove any pre-release version suffixes

**Update spec file:**
- Remove `%global prerelease` line if present
- Update `Release:` field: change from `0.pre.N%{?dist}` to `1%{?dist}` for GA
- Verify `Version:` matches GA version

### 4. Update ecr-credential-provider Package

```bash
cd ../ecr-credential-provider-${VER_NODOT}
```

**Update Cargo.toml:**
- Change source URLs to GA release
- Update checksums

**Update clarify.toml:**
- Update license hashes if sources changed

```bash
vim clarify.toml
```

### 5. Build and Validate

Use the `build-kit-locally` skill to build and publish the updated core-kit:

```bash
# See skills/build-kit-locally/SKILL.md
# This will build both packages and publish to local registry
```

## Validation

Success indicators:
- Both packages build without errors (verified by build-kit-locally skill)
- License checks pass (clarify.toml hashes correct)
- Source downloads succeed (GA URLs valid)
- Kit successfully published to local registry

## Common Issues

**Source URL 404:** GA not yet released. Wait for upstream release.

**Checksum mismatch:** Re-download and recalculate checksums.

**License hash mismatch:** Source tarball contents changed. Update clarify.toml hashes.

## Reference

- [Kubernetes Releases](https://github.com/kubernetes/kubernetes/releases)
- [EKS-D Releases](https://github.com/aws/eks-distro/releases)
