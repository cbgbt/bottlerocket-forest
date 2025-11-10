# Bottlerocket Architecture Overview

## Core Concepts

### Kits

Kits are collections of packages that provide functionality for Bottlerocket:

- **core-kit**: Essential OS packages (systemd, containerd, networking, etc.)
- **kernel-kit**: Linux kernel and kernel modules

Kits are built independently and published to OCI registries. They are versioned and can be consumed by multiple variants.

### Variants

Variants are complete Bottlerocket OS images tailored for specific use cases:

- Different orchestrators (Kubernetes, ECS)
- Different platforms (AWS, bare metal, VMware)
- Different architectures (x86_64, aarch64)

Examples: `aws-k8s-1.31`, `aws-ecs-2`, `metal-k8s-1.31`

Variants consume kits and add variant-specific configuration and packages.

### Build Flow

```
┌─────────────┐
│  core-kit   │──┐
└─────────────┘  │
                 │  Build & Publish to OCI
┌─────────────┐  │
│ kernel-kit  │──┤
└─────────────┘  │
                 ▼
            ┌──────────┐
            │   OCI    │
            │ Registry │
            └──────────┘
                 │
                 │  Pull kits
                 ▼
            ┌──────────┐
            │ Variant  │
            │  Build   │
            └──────────┘
                 │
                 ▼
            ┌──────────┐
            │  Image   │
            │  (.img)  │
            └──────────┘
```

### Twoliter

Twoliter is Bottlerocket's build tool. It:
- Builds kits and variants using the Bottlerocket SDK
- Manages dependencies between packages
- Publishes artifacts to OCI registries
- Handles cross-compilation and architecture-specific builds

### SDK

The Bottlerocket SDK provides the build environment:
- Toolchain (compiler, linker, etc.)
- Build dependencies
- Consistent build environment across developers

## Development Workflow

1. **Make changes** to packages in a kit (e.g., core-kit)
2. **Build the kit** using twoliter
3. **Publish kit** to OCI registry (local or remote)
4. **Update variant** to reference new kit version
5. **Build variant** to create bootable image
6. **Test image** in target environment

## Key Files

- `Twoliter.toml` - Project configuration (in kit/variant repos)
- `Cargo.toml` - Rust package manifests
- `*.spec` - RPM package specifications
- `Makefile.toml` - Build instructions for packages

## Further Reading

- [Bottlerocket Documentation](https://bottlerocket.dev/)
- [Twoliter Documentation](../twoliter/docs/)
- Component-specific READMEs in each repository
