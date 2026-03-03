---
name: build-package
description: Build a third party package in a Bottlerocket kit (core-kit or kernel-kit)
---

# Skill: Build and Publish Kit

## Purpose

Build a third party package in a Bottlerocket kit (core-kit or kernel-kit). This allows you to test changes in a third party package spec file or Cargo.toml without publishing to production registries. You can use it while updating or modifying a package.

## When to Use

- After modifying a package spec
- After updating a package version
- Debugging a package build failure
- Iterating on patches or spec file changes

## Prerequisites

- Docker installed and running
- Working from within a grove directory
- Kit repository cloned in `kits/` directory (e.g., `kits/bottlerocket-core-kit`)

## Input

The user provides:
- **Package name** (e.g. `libselinux`)
- **Kit name** (e.g. `bottlerocket-core-kit`)

## Procedure

### 1. Locate the package

```bash
ls kits/<kit-name>/packages/<package-name>/
```

Some packages are versioned (e.g., `kubernetes-1.29`, `ecr-credential-provider-1.30`).
If the user specifies a versioned package, locate the correct directory.

### 2. Navigate to the kit

```bash
cd kits/<kit-name>
```

### 3. Build the package

```bash
PACKAGE=<package-name> make twoliter build-package
```

## Common Issues

**Format error**
```
error: invalid character `.` in package name:
```
Solution: Replace `.` with `_` in package name and retry.

**Package is not present in bottlerocket cache:**
```
Failed to fetch `https://cache.bottlerocket.aws
```
Solution: Run the following
```bash
PACKAGE=<package-name> make twoliter build-package -e BUILDSYS_UPSTREAM_SOURCE_FALLBACK=true
```

**Build takes too long:**
Single-package builds are much faster than full kit builds, but some packages (e.g., kernel) are inherently slow.

**Dependency not yet built:**
If the package depends on another package that hasn't been built yet, you may need to build the dependency first or do a full kit build.

**Twoliter not found:**
Ensure you're in the kit directory and the Makefile/twoliter is properly configured.

**Docker permission denied:**
Solution: Ensure user is in docker group and Docker daemon is running
