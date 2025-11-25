---
name: build-variant-from-local-kits
description: Build a variant using locally published kits for development validation
---

# Skill: Build Variant from Local Kits

## Purpose

Build a complete Bottlerocket variant image using kits published to the local development registry. This enables end-to-end testing of kit changes before publishing to production registries.

## When to Use

- Testing kit changes in a complete variant build
- Creating bootable images for local testing
- End-to-end validation of kit modifications

## Prerequisites

- Kits already built and published to local registry (use `build-kit-locally` skill)
- Local registry running
- Bottlerocket variant repository

## Procedure

### 1. Update variant Twoliter.toml

Edit `bottlerocket/Twoliter.toml` to reference local kits:

```toml
[[kit]]
name = "bottlerocket-core-kit"
version = "<version-from-kit>"
vendor = "local"

[[kit]]
name = "bottlerocket-kernel-kit"
version = "<version-from-kit>"
vendor = "local"
```

### 2. Configure Infra.toml

Ensure `bottlerocket/Infra.toml` includes local registry:

```toml
[vendor.local]
registry = "localhost:5000"
```

### 3. Update lock file

```bash
cd bottlerocket
./tools/twoliter/twoliter update
```

### 4. Build the variant

```bash
cargo make
```

For specific variant:
```bash
cargo make -e BUILDSYS_VARIANT=aws-k8s-1.31
```

For specific architecture:
```bash
cargo make -e BUILDSYS_ARCH=aarch64
```

### 5. Locate the built image

```bash
ls -lh build/images/*.img
```

## Validation

The build should complete successfully and produce an `.img` file in `build/images/`.

## Common Issues

**Kit not found in registry:**
```
Error: failed to pull kit
```
Solution: Verify kit is published with `curl http://localhost:5000/v2/_catalog`

**Version mismatch:**
Solution: Ensure `Twoliter.toml` version matches the published kit version

**Lock file out of sync:**
Solution: Run `./tools/twoliter/twoliter update` again
