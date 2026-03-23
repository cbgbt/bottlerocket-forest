---
name: update-package
description: Update a third-party package to a new version in Bottlerocket kit (core-kit or kernel-kit)
---

# Skill: Update Third Party Package

## Purpose

Update a package to a new upstream version within a Bottlerocket kit (core-kit or kernel-kit). This includes updating the the Cargo.toml - source URL and hash, if present the signature's source URL and hash, the RPM spec file, and re-applying or re-creating patches as needed.

## When to Use

- Updating a third-party package to a new upstream release

## Prerequisites

- Working from within a grove directory
- Kit repository cloned in `kits/` directory (e.g., `kits/bottlerocket-core-kit`)

## Input

The user provides:
- **Package name(s) and the version to update to** (e.g., `libselinux=3.7` or `libselinux=3.7 libsepol=3.7`)
- **Kit name** (e.g. `bottlerocket-core-kit`)

## Procedure

### 1. Locate the package

```bash
ls kits/<kit-name>/packages/<package-name>/
```

Some packages are versioned (e.g., `kubernetes-1.29`, `ecr-credential-provider-1.30`).
If the user specifies a versioned package, locate the correct directory.

Identify the key files:
- `Cargo.toml` — source URL(s) and SHA-512 hashes for the package and if present the signatures
- `<package-name>.spec` — RPM spec with Version, Release, %prep, %build, %install, %changelog
- `*.patch` — any existing patches

Read all of them before making changes.

### 2. Navigate to the kit

```bash
cd kits/<kit-name>
```

### 3. Update Cargo.toml

Open `packages/<package-name>/Cargo.toml`.

Use the url field in `[[package.metadata.build-package.external-files]]` sections and replace the version to point to the new version. The URL pattern varies by package — match the existing pattern but replace the old version with the new one.

If there is a `path` field in the same section (overrides the downloaded filename), update the version in it too.

To get the new hash, download the source and compute it:

```bash
TMPDIR=$(mktemp -d)
curl -L -o "$TMPDIR/source.tar.gz" "<new-url>"
sha512sum "$TMPDIR/source.tar.gz" | awk '{print $1}'
rm -rf "$TMPDIR"
```

If there are multiple `[[package.metadata.build-package.external-files]]` entries (e.g., separate source tarballs, signature files), update each one that embeds the version.

The `releases-url` field at the top of the build-package metadata section points to the upstream release page — use it to verify the new version exists and find the correct download URL if the pattern isn't obvious.

In some cases, patch files have to be appended. Follow the format of the Cargo.toml to add new patch files from the upstream source.

#### Kubernetes packages

In case of kubernetes packages, the URL contains both the release number (eg. `releases/12/`) and the kubernetes version (eg. `v1.34.3`). Release version is usually the one that's provided. To find the correct patch number, check the EKS distro releases page or try incrementing the patch number until the URL resolves.

### 3. Update the spec file

Open `packages/<package-name>/<package-name>.spec`.

**Read the spec file carefully first** to identify which versioning pattern it uses.
Different packages use different macro patterns — you must match the existing pattern.

Update these fields:
- `Version:` — set to the new version (unless a macro derives it — see below)
- `Release:` — reset to `1%{?dist}` (unless it uses a special pattern like `%{ncurses_rev}%{?dist}`)
- `Source{xx}:` (Source lines) — update filenames if they embed the version or a file was replaced by a new one (eg. GPG key)

DO NOT CHANGE THE `Epoch:` field if present.

#### Spec versioning patterns

Packages use various macro patterns for versioning. Always check which pattern the spec uses:

- **Direct `Version:`** — most common. Just update the `Version:` field.
- **`%global gover`** — Go packages. Update `gover`, leave `Version:` alone.
- **`%global unversion`** — libexpat style. Uses underscores (e.g., `2_7_2`). Update with underscored new version. `Version:` is derived via sed.
- **`%global ncurses_rev`** — libncurses. The date portion (e.g., `20250927`). `Version:` stays as the major version. `Release:` uses `%{ncurses_rev}%{?dist}` — do not reset it.
- **`%global releasever`** — kubernetes. The EKS release number. See Kubernetes section below.
- **`%global majorminor` + `%global version`** — util-linux style. `majorminor` is like `2.41`, `version` is like `%{majorminor}.2`. Update the patch number in `%global version`.

Note: Some packages use a different version format in the spec `Version:` field than in the URL.
For example, libinih uses `Version: 60` in the spec but `r60` in the URL.
When updating, ensure both the spec version and the URL version are updated consistently with their respective formats.

#### Kubernetes packages

The spec has these 2 global variables `gover`, `releasever` defined as follows:
```spec
%global releasever <release-ver>
%global gover <kubernetes-version>
```
Just change the value of the `releasever` variable and if you had to bump the patch version while updating `Cargo.toml` then `gover` instead of `Version:` field in this case. If the Source references the global variable `gover` then no need to change.

#### Other Go packages

The spec may have a global variable `gover` defined as follows:
```spec
%global gover NEW_VERSION
```
Just change the value of this variable instead of `Version:` field in this case. If the Source references the global variable `gover` then no need to change.

Check whether the upstream bumped its Go version.
If so, update the `GO_MAJOR` export in the `%build` section of the spec:

```spec
export GO_MAJOR="1.25"
```

Only set this if the upstream project actually requires the newer Go version.
If the build system's default Go version now matches or exceeds what upstream requires, **remove** the `GO_MAJOR` export line (and any trailing blank line after it) instead of updating it.

#### Packages with incremental patches

Some packages (e.g., readline) release incremental patches rather than full tarballs for minor updates.
These patches are available from the upstream distribution site (e.g., `https://ftp.gnu.org/gnu/readline/readline-8.3-patches/`).
Match the existing Cargo.toml pattern for how these are pulled in. If a new set of patches are added, make sure a .gitignore file is also
added so that those patches don't show up as uncommitted. And do not commit those patches.

### 4. Build the package

Use SKILL `build-package` to build `<package-name>` in `<kit-name>`.
In case of multiple packages build them one by one.

## Validation

After making all changes, verify:
- [ ] `Cargo.toml` has correct URL(s) and valid SHA-512 hash(es)
- [ ] Spec file `Version:` matches the new version
- [ ] Spec file `Release:` is reset to `1%{?dist}`
- [ ] All `Source` lines reference correct filenames
- [ ] Patches are either removed (if upstreamed) or kept with correct numbering
- [ ] No stale patch references remain in the spec
- [ ] GPG keys are current if upstream rotated signing keys
- [ ] Package successfully builds using `build-package` skill

## Commit Messages

If the Validation passes then create the commit.
Follow the project convention: `packages: update <package-name> to <version>`

Note removed patches in the commit body (e.g., "removed upstreamed patch for X").

```
packages: update nvidia-k8s-device-plugin to v0.18.0

Signed-off-by: Your Name <your-email>
```

## Common Issues

**Signing Verification failed during package build:**
```
gpgverify: Decoding the keyring failed.
```
Solution: The GPG key used for signature verification has been rotated by the upstream project.

1. Find the key ID used to sign the new release (check the `.asc`/`.sig` file or upstream release notes)
2. Download the new public key from a keyserver or the upstream project
3. Save it as `gpgkey-<FINGERPRINT>.asc` in the package directory
4. Delete the old `gpgkey-*.asc` file
5. Update the `Source{xx}:` line in the spec to reference the new key filename
6. Update the Cargo.toml if the key file is listed as an external-file

**Missing Dependency:**

Check if the new version has changed build requirements.
If so, update `BuildRequires:` in the spec file and any path dependencies in `Cargo.toml`:

```toml
[build-dependencies]
some-other-package = { path = "../some-other-package" }
```

This also requires update to `sources/Cargo.lock`. To do that just run `cargo build` inside `sources` directory.

**Patch application failure:**
For the problematic patch:
1. Read the patch to understand what it modifies
2. Determine if the fix has been upstreamed in the new version:
   - If upstreamed: remove the patch file and its `PatchNNNN:` and `%patch` lines from the spec
   - If still needed: keep it, but note it may need rebasing
   - If uncertain: keep it for now — the build step will reveal if it applies cleanly

To check if a patch was upstreamed, look at the upstream changelog or release notes for the new version.
If the patch was originally created from an upstream commit (check the `From` header in the patch), search for that commit in the new release.

If rebase is required for the patch, then prefer `git format-patch` from the upstream repo to preserve the original commit reference:

```bash
git clone <upstream-repo>
cd <upstream-repo>
git format-patch -1 <commit-hash>
```

This keeps provenance and makes review easier.

Use standard `git format-patch` style numbering (0001-, 0002-, etc.) and add corresponding `PatchNNNN:` and `%patch -P NNNN -p1` lines in the spec.

If a new version introduces behavior that needs a Bottlerocket-specific fix, document why the patch is needed in the commit message using cite-style links:

```
as suggested in [1]

[1]: https://example.com/relevant-issue
```
