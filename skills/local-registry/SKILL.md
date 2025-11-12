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
- Forester tool built (`cd forester && make release-build`)

## Procedure

### Start the registry

```bash
./forester/target/release/forester registry start
```

This will:
- Start a local Docker registry on `localhost:5000`
- Configure persistence (registry data survives restarts)
- Wait for the registry to be healthy before returning
- Output the registry URL for use in builds

### Verify registry is running

```bash
./forester/target/release/forester registry status
```

Returns exit code 0 if running, non-zero otherwise.

### View registry logs

```bash
./forester/target/release/forester registry logs
```

To follow logs in real-time:
```bash
./forester/target/release/forester registry logs --follow
```

### Stop the registry

```bash
./forester/target/release/forester registry stop
```

Note: This preserves the registry data volume.

### Clean registry data

```bash
./forester/target/release/forester registry clean
```

This removes both the container and the data volume.

## Configuration

Forester uses environment variables for configuration. Create a `.env` file in the forester directory:

```bash
# Custom port (default: 5000)
FORESTER_REGISTRY_PORT=5001

# Custom container name (default: forester-registry-{port})
FORESTER_REGISTRY_CONTAINER_NAME=my-registry

# Custom volume name (default: forester-registry-data-{port})
FORESTER_REGISTRY_VOLUME_NAME=my-registry-data

# Custom image (default: registry:2)
FORESTER_REGISTRY_IMAGE=registry:2.8
```

## Validation

After starting the registry:
```bash
curl http://localhost:5000/v2/_catalog
```

Should return: `{"repositories":[]}`

## Common Issues

**Docker not installed:**
```
Error: Docker is not installed or not in PATH. Install Docker and ensure it's in your PATH
```
Solution: Install Docker and ensure it's in your PATH.

**Docker daemon not running:**
```
Error: Docker daemon is not running. Start Docker with: sudo systemctl start docker
```
Solution: Start the Docker daemon.

**Port already in use:**
```
Error: Failed to start container. The port may already be in use
```
Solution: 
- Check for existing registry: `docker ps | grep registry`
- Stop conflicting container: `docker stop <container-id>`
- Or use a custom port via `FORESTER_REGISTRY_PORT`

**Permission denied:**
- Ensure user is in docker group: `groups | grep docker`
- Add user to docker group: `sudo usermod -aG docker $USER`
- May need to restart shell after adding to group

## Related Skills

- `build-and-publish-kit` - Uses local registry to publish built kits
- `build-variant` - Configures variant builds to use local registry

