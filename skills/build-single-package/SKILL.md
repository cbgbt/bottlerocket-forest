---
name: build-single-package
description: Build a single package in a kit to test changes without building the entire kit
context: fork
---

# Build Single Package

Build and verify a single package within a Bottlerocket kit for rapid iteration during development.

## When to Use

- Testing changes to a package spec file
- Debugging package build failures
- Iterating on package patches
- Verifying package dependencies resolve

## Prerequisites

- Working in a kit directory (bottlerocket-core-kit or bottlerocket-kernel-kit)
- Package spec exists in kit's packages directory

## Workflow

1. **Identify Package and Kit**
   - Determine which kit contains the package
   - Navigate to kit directory

2. **Build Package**
   - Run `<grove-root>/skills/build-single-package/scripts/build-single-package -p <package-name> [-a <arch>]`
   - Use absolute path from grove root
   - `-p` flag specifies the package name (required)
   - `-a` flag specifies target architecture (optional)
   - Defaults to host architecture (detected via `uname -m`) if not specified
   - This builds only the specified package and its direct dependencies

3. **Verify Build Success**
   - Check for RPM artifacts in `build/rpms/<package-name>/`
   - Confirm expected package files exist
   - Report build output and artifact locations

## Notes

- Package must be defined in the kit's packages/ directory
- Build artifacts are placed in kit's build/rpms/<package-name>/ directory
- Script detects host architecture automatically using `uname -m`
- Override with `-a` flag to cross-compile
- Supported architectures: x86_64, aarch64

## Related Skills

- `patch-third-party-package` - Create patches for packages
