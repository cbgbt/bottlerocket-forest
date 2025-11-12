# Bottlerocket Build System

## Overview

Each Bottlerocket repository uses **Makefiles** as the primary build interface. The Makefiles wrap `twoliter` commands and handle dependency management.

Note that some repos use GNU make (Makefile) and some use cargo-make (Makefile.toml).

**Do not invoke `twoliter` directly** - always use the Makefile targets.

## Kit Builds (core-kit, kernel-kit)

### Makefile Targets

```bash
# Build the kit for current architecture
make build

# Build for specific architecture
make build ARCH=x86_64
make build ARCH=aarch64

# Fetch dependencies
make fetch

# Update Twoliter.lock
make update

# Publish to registry
make publish VENDOR=my-vendor

# Access twoliter make tasks
make twoliter <task-name>
```

### Build Flow

1. **prep** - Installs twoliter if needed
2. **fetch** - Downloads dependencies
3. **build** - Builds the kit (wraps `twoliter build kit`)
4. **publish** - Publishes to OCI registry (wraps `twoliter publish kit`)

### Configuration

- `Twoliter.toml` - Project configuration
- `Infra.toml` - Registry/vendor configuration
- `Makefile` - Build targets and twoliter wrapper

## Variant Builds (bottlerocket/)

### Using cargo-make

The main Bottlerocket repository uses `cargo-make` with `Makefile.toml`:

```bash
# Build default variant (aws-k8s-1.32)
cargo make

# Build specific variant
cargo make -e BUILDSYS_VARIANT=aws-k8s-1.31

# Build for specific architecture
cargo make -e BUILDSYS_ARCH=aarch64

# Build and register AMI
cargo make -e PUBLISH_REGIONS=us-west-2 ami

# Limit build concurrency
cargo make -e BUILDSYS_JOBS=4
```

### Key Variables

- `BUILDSYS_VARIANT` - Which variant to build (default: aws-k8s-1.32)
- `BUILDSYS_ARCH` - Target architecture (default: current arch)
- `BUILDSYS_JOBS` - Build parallelism (default: 8)
- `PUBLISH_REGIONS` - AWS regions for AMI registration

## Development Workflow

### 1. Build a Kit

```bash
cd kits/bottlerocket-core-kit
make build ARCH=x86_64
```

### 2. Publish to Local Registry

```bash
# Configure Infra.toml with your registry
make publish VENDOR=my-vendor
```

### 3. Update Variant to Use New Kit

Edit `bottlerocket/Twoliter.toml`:
```toml
[[kit]]
name = "bottlerocket-core-kit"
version = "2.x.y"
vendor = "my-vendor"
```

Update the lock file:
```bash
cd bottlerocket
./tools/twoliter/twoliter update
```

### 4. Build Variant

```bash
cd bottlerocket
cargo make -e BUILDSYS_VARIANT=aws-k8s-1.32
```

## Common Patterns

### Building Everything from Scratch

```bash
# Build core-kit
cd kits/bottlerocket-core-kit
make build

# Build kernel-kit
cd ../bottlerocket-kernel-kit
make build

# Build variant (uses published kits)
cd ../../bottlerocket
cargo make
```

### Iterative Development

When working on a kit:
1. Make changes to packages
2. `make build` - Only changed packages rebuild
3. `make publish VENDOR=dev` - Publish to dev registry
4. Update variant's `Twoliter.toml` version
5. `cargo make` in variant repo

### Using Upstream Source Fallback

For packages with restricted sources (e.g., NVIDIA):
```bash
make build UPSTREAM_SOURCE_FALLBACK=true
```

## Registry Configuration

### Infra.toml Format

```toml
[vendor.my-vendor]
registry = "123456789.dkr.ecr.us-west-2.amazonaws.com"

[vendor.public]
registry = "public.ecr.aws/bottlerocket"
```

### Twoliter.toml Kit Dependencies

```toml
[[kit]]
name = "bottlerocket-core-kit"
version = "2.0.0"
vendor = "my-vendor"

[[kit]]
name = "bottlerocket-kernel-kit"
version = "1.0.0"
vendor = "public"
```

## Build Artifacts

### Kits
- Built as OCI images
- Published to container registries
- Versioned and immutable

### Variants
- Output: `build/images/*.img`
- Bootable disk images
- Include all kit contents

## Dependencies

### System Packages

**Ubuntu:**
```bash
apt install build-essential openssl libssl-dev pkg-config liblz4-tool cmake clang libclang-dev
```

**Fedora:**
```bash
dnf install make automake gcc openssl openssl-devel pkg-config lz4 perl-FindBin perl-lib cmake clang
```

### Rust Tools

```bash
# Install rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install cargo-make (for variant builds)
cargo install cargo-make
```

### Container Tools

**Option 1: Docker** (20.10.10+)
- Enable BuildKit and containerd-snapshotter in `/etc/docker/daemon.json`

**Option 2: Crane** (recommended)
- Faster, no daemon required
- Install from https://github.com/google/go-containerregistry

## Troubleshooting

### "twoliter not found"
Run `make prep` to install twoliter.

### "kit not found in registry"
Ensure `Infra.toml` is configured and you've run `make publish`.

### Build cache issues
Twoliter caches package builds. To force rebuild:
```bash
# Clean and rebuild
rm -rf build/
make build
```

### Docker permission denied
Add your user to the docker group:
```bash
sudo usermod -aG docker $USER
# Log out and back in
```
