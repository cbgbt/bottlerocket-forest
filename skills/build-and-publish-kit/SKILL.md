---
name: build-and-publish-kit
description: Build a Bottlerocket kit and publish it to the local development registry
---

# Skill: Build and Publish Kit

## Purpose

Build a Bottlerocket kit (core-kit or kernel-kit) and publish it to a local OCI registry for development and testing.

## When to Use

- Making changes to kit packages and testing them in variants
- Iterative development on kits
- Before building a variant image that depends on kit changes

## Prerequisites

- Forester tool built
- Docker installed and running
- Kit repository cloned in `kits/` directory

## Procedure

### 1. Ensure local registry is running

```bash
./forester/target/release/forester registry start
```

### 2. Configure Infra.toml for local registry

Check if `Infra.toml` exists in the kit directory. If not, create it:

```bash
cd kits/<kit-name>
cat > Infra.toml << 'EOF'
[vendor.local]
registry = "localhost:5000"
EOF
```

### 3. Build the kit

```bash
make build
```

For specific architecture:
```bash
make build ARCH=x86_64
# or
make build ARCH=aarch64
```

### 4. Publish to local registry

```bash
make publish VENDOR=local
```

This publishes the kit to `localhost:5000` with the vendor prefix "local".

### 5. Verify publication

```bash
curl http://localhost:5000/v2/_catalog
```

Should show your kit in the repositories list.

## Validation

Check the kit is available:
```bash
curl http://localhost:5000/v2/<kit-name>/tags/list
```

Should return the published version tags.

## Common Issues

**Registry not running:**
```
Error: connection refused
```
Solution: Run `./forester/target/release/forester registry start`

**Infra.toml not configured:**
```
Error: vendor 'local' not found
```
Solution: Create or update `Infra.toml` with local registry configuration

**Docker permission denied:**
Solution: Ensure user is in docker group and Docker daemon is running

## Next Steps

After publishing a kit:
1. Update variant's `Twoliter.toml` to reference the new kit version
2. Run `./tools/twoliter/twoliter update` in the variant repo
3. Build the variant with `cargo make`
