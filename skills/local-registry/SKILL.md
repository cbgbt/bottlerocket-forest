---
name: local-registry
description: Start and manage a local OCI registry for Bottlerocket kit development
---

# Skill: Local OCI Registry

## Purpose

Start and manage a local OCI registry for development. This allows building and publishing kits locally without requiring external registry access.

## When to Use

- Developing changes to kits that need to be consumed by variants
- Testing kit changes before publishing to production registries
- Working offline or in isolated environments

## Prerequisites

- Docker installed and running
- Forest tool built (`cd forester && cargo build --release`)

## Procedure

### Start the registry

```bash
forest registry start
```

This will:
- Start a local Docker registry on `localhost:5000`
- Configure persistence (registry data survives restarts)
- Output the registry URL for use in builds

### Verify registry is running

```bash
forest registry status
```

### Stop the registry

```bash
forest registry stop
```

### Clean registry data

```bash
forest registry clean
```

## Validation

After starting the registry:
```bash
curl http://localhost:5000/v2/_catalog
```

Should return: `{"repositories":[]}`

## Common Issues

**Port 5000 already in use:**
- Check for existing registry: `docker ps | grep registry`
- Stop conflicting container: `docker stop <container-id>`

**Permission denied:**
- Ensure user is in docker group: `groups | grep docker`
- May need to restart shell after adding to group

## Related Skills

- `build-and-publish-kit` - Uses local registry to publish built kits
- `build-variant` - Configures variant builds to use local registry
