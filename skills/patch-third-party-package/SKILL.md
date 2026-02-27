---
name: patch-third-party-package
description: Create patches for Bottlerocket third-party packages by extracting sources, making changes, generating patch files, and updating RPM specs
---

# Patch Third-Party Package

## Purpose

Create patches that modify third-party packages in Bottlerocket to adjust paths, fix compatibility issues, or backport fixes. This skill guides you through fetching sources, making changes, generating git-formatted patches, and integrating them into the RPM build system for Bottlerocket

## When to Use

- Need to adjust filesystem paths for Bottlerocket's layout
- Fix build system compatibility issues (e.g., musl vs glibc)
- Modify package behavior for Bottlerocket-specific requirements
- Add Bottlerocket-specific configuration or features

## Prerequisites

- Running from within a grove (not forest root)
- Target package exists in `kits/*/packages/`

## Procedure

### 1. Locate Package

Find the package directory:

```bash
find kits/*/packages -name "PACKAGE_NAME" -type d
```

### 2. Read Source Metadata

Extract source URLs and checksums from the package's `Cargo.toml`:

```bash
.<grove-root>/skills/patch-third-party-package/scripts/extract-source-metadata PACKAGE_NAME
```

This displays the source tarball URL and its sha512 checksums.
Each external file has its own entry with URL and sha512 checksum.

### 3. Fetch and Verify Sources

Create temporary workspace and fetch sources:

```bash
WORK_DIR=$(mktemp -d -t patch-PACKAGE-XXXX)
cd $WORK_DIR

# Fetch sources
curl -LO SOURCE_URL

# Verify sha512 checksum
echo "SHA512_FROM_CARGO  filename" | sha512sum -c -
```

### 4. Extract Sources

Read the package's RPM spec file to understand how sources should be extracted:

```bash
cat kits/*/packages/PACKAGE_NAME/PACKAGE_NAME.spec
```

Look at the `%prep` section and follow its pattern exactly. Common patterns:

**Simple autosetup** (most common):
```spec
%autosetup -n package-%{version} -p1
```

**Git-based autosetup**:
```spec
%autosetup -Sgit -n package-%{version} -p1
```

**SRPM extraction** (grub, kernel):
```spec
rpmkeys --import %{S:1} --dbpath "${PWD}/rpmdb"
rpmkeys --checksig %{S:0} --dbpath "${PWD}/rpmdb"
rm -rf "${PWD}/rpmdb"
rpm2cpio %{S:0} | cpio -iu '*.tar.xz' '*.patch'
tar -xof package.tar.xz
rm package.tar.xz
%setup -TDn package-%{version}
```

Extract the sources following the pattern from the spec file.

### 5. Initialize Git Repository

After extraction, initialize git in the source directory:

```bash
cd EXTRACTED_SOURCE_DIR
git init
git add .
git commit -S -s -m "Initial import of PACKAGE_NAME sources"
```

### 6. Make Changes

Create the necessary changes. Commit each logical change separately:

```bash
# Make your changes
git add -A
git commit -S -s -m "fix: adjust paths for Bottlerocket filesystem layout"

# Make additional changes if needed
git add -A
git commit -S -s -m "build: disable feature incompatible with musl"
```

Each commit message should explain **why** the change is needed, not what code changed.

### 7. Generate Patches

Generate git-formatted patches from your commits:

```bash
# Generate patches (excluding the initial import commit)
git format-patch --no-numbered --no-signature -N HEAD~N
```

Where N is the number of commits to convert to patches. This creates files like:
- `0001-fix-adjust-paths-for-bottlerocket.patch`
- `0002-build-disable-feature-incompatible.patch`

### 8. Copy Patches to Package

Copy the generated patches to the package directory:

```bash
cp *.patch GROVE_ROOT/kits/*/packages/PACKAGE_NAME/
```

### 9. Update RPM Spec

Add patch references to the spec file header:

```spec
Patch0001: 0001-fix-adjust-paths-for-bottlerocket.patch
Patch0002: 0002-build-disable-feature-incompatible.patch
```

Verify the `%prep` section will apply them. Most packages use `%autosetup` which automatically applies all numbered patches:

```spec
%prep
%autosetup -n PACKAGE_NAME-%{version} -p1
```

If the package uses a different method, ensure your patches will be applied:
- `%autopatch -p1`: Applies patches after manual setup
- `git am` with `-Sgit`: For git-formatted patches
- Manual `patch -p1` loop: When order matters

### 10. Commit Changes

Commit the patches and spec file changes to the repository:

```bash
cd GROVE_ROOT/REPO_NAME
git add .
git commit -S -s -m "Git commit message"
```

## Validation

Build and test the package:
1. Build the package using existing build skills
2. Verify the package builds successfully

## Common Issues

**Patch fails to apply during build:**
- Verify patch was generated with correct `-p` level (usually `-p1`)
- Check that `%prep` section uses matching `-p` level
- Ensure patches are numbered sequentially starting from 0001

**Source extraction fails:**
- Verify sha512 checksums match Cargo.toml
- Check that extraction commands match the spec file's `%prep` section exactly

## Next Steps

After creating patches:
- Use **build-kit-locally** or similar skills to build and test the package
