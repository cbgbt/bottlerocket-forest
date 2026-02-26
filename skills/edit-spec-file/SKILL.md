---
name: edit-spec-file
description: Edit Bottlerocket RPM spec files following project style conventions
---

# Skill: Edit Spec File

## Purpose

Make changes to Bottlerocket RPM spec files while maintaining strict compliance with project style conventions. This skill ensures consistent, maintainable spec files across all kit packages.

## When to Use

- Adding new source files, patches, or templates to a package
- Adding systemd services or configuration files
- Creating new subpackages (including FIPS variants)
- Modifying build or install sections
- Any modification to a `.spec` file in `kits/*/packages/`

## Prerequisites

- Read `./STYLE-GUIDE.md` completely before making changes
- Identify an existing similar package as a reference pattern

## Procedure

### 1. Read the Style Guide

**MANDATORY**: Read `./STYLE-GUIDE.md` before making any changes.

The style guide contains:
- [MUST] rules that are mandatory
- [MUST NOT] rules that are prohibited
- [SHOULD] rules that are strongly recommended

### 2. Find a Reference Package

Identify one or several similar existing package(s) to use as a pattern:

```bash
# For Go packages
ls kits/bottlerocket-core-kit/packages/*/nvidia-k8s-device-plugin.spec

# For C packages with systemd services
ls kits/bottlerocket-core-kit/packages/chrony/chrony.spec

# For packages with FIPS variants
ls kits/bottlerocket-core-kit/packages/amazon-ssm-agent/amazon-ssm-agent.spec
```

### 3. Apply Changes Following Section Order

Spec files have a strict section order:

1. `%global` definitions
2. Package metadata (Name, Version, Release, Summary, License, URL)
3. Source files (numbered by range: <100 misc, 1xx systemd, 2xx tmpfiles, 3xx udev, 4xx licenses)
4. Patches (numbered 1000+)
5. BuildRequires / Requires
6. `%description`
7. Subpackage definitions (`%package`, `%description`)
8. `%prep`
9. `%build`
10. `%install`
11. `%files` sections

### 4. Source File Numbering

Follow the established ranges if needed, use the example packages for specific guidance above the following examples:
- `Source0-Source99`: Main sources and miscellaneous files
- `Source100-Source199`: Systemd unit files
- `Source200-Source299`: Tmpfiles configurations
- `Source300-Source399`: Udev rules
- `Source400-Source499`: License files

**CRITICAL**: When removing sources, leave gaps - do NOT renumber existing sources unless its in the same hundreds section.

### 5. Install Files in Source Order

Files in `%install` must be installed in the same order as their Source declarations:

```spec
# Sources declared as:
Source1: nvidia-k8s-device-plugin.service
Source2: nvidia-k8s-device-plugin-conf
Source3: nvidia-k8s-device-plugin-exec-start-conf

# Install in same order:
install -p -m 0644 %{S:1} %{buildroot}%{_cross_unitdir}
install -D -m 0644 %{S:2} %{buildroot}%{_cross_templatedir}/nvidia-k8s-device-plugin-conf
install -D -m 0644 %{S:3} %{buildroot}%{_cross_templatedir}/nvidia-k8s-device-plugin-exec-start-conf
```

### 6. Use Correct Macros

Always use cross-compilation macros:
- `%{_cross_bindir}` not `/usr/bin`
- `%{_cross_unitdir}` for systemd units
- `%{_cross_templatedir}` for templates
- `%{_cross_factorydir}%{_cross_sysconfdir}/...` for factory defaults
- `%{_cross_fips_bindir}` for FIPS binaries

### 7. FIPS Subpackage Pattern

When adding binaries that need FIPS variants:

```spec
%package bin
Summary: Package binaries
Provides: %{name}(binaries)
Requires: (%{_cross_os}image-feature(no-fips) and %{name})
Conflicts: (%{_cross_os}image-feature(fips) or %{name}-fips-bin)

%description bin
%{summary}.

%package fips-bin
Summary: Package binaries, FIPS edition
Provides: %{name}(binaries)
Requires: (%{_cross_os}image-feature(fips) and %{name})
Conflicts: (%{_cross_os}image-feature(no-fips) or %{name}-bin)

%description fips-bin
%{summary}.
```

### 8. File Permissions

Use consistent permissions:
- `0755` for executables
- `0644` for config files, templates, service files
- Use `install -p` to preserve timestamps
- Use `install -D` when parent directories need creation

## Validation Checklist

Before committing, spawn a subagent to independently verify, refer to this document and the 'Validation Checklist':

- [ ] Source files numbered in correct ranges
- [ ] Install order matches Source declaration order
- [ ] All paths use `%{_cross_*}` macros
- [ ] No files installed to `/etc` or `/var` (use factory defaults pattern)
- [ ] `%{_cross_attribution_file}` in main `%files` section
- [ ] `%license` directive for license files
- [ ] `%dir` directive for directories owned by package
- [ ] No debugging commands (`ls -la`, etc.)
- [ ] No unnecessary comments
- [ ] Consistent formatting (no trailing whitespace, consistent indentation)
- [ ] `Release: 1%{?dist}` format (no dot before `%`)

## Committing Changes

When committing spec file changes, use `-F` with a file to preserve newlines in commit messages:

```python
# Write message to file, then commit with -F
msg = """package-name: summary of change

Detailed description of what changed and why.

Signed-off-by: Name <email>"""
write("create", "/tmp/commit-msg.txt", file_text=msg)
bash("git -c commit.gpgsign=false commit -F /tmp/commit-msg.txt")
```

**DO NOT** use `git commit -m` with `repr()` or escaped strings - this writes literal `\n` instead of newlines.

## Common Anti-Patterns

**DO NOT:**
- Use `Release: 1.%{?dist}` (wrong: has dot)
- Include `./` prefix in file paths
- Add comments that restate obvious operations
- Use `%exclude` as substitute for proper build configuration
- Renumber sources when removing items
- Install files out of source order
- Use hardcoded paths instead of macros
- Create cross-package symlinks

## Quick Reference

### Adding a Systemd Service

1. Add source in 100-range: `Source101: myservice.service`
2. Install to unitdir: `install -p -m 0644 %{S:101} %{buildroot}%{_cross_unitdir}`
3. Add to %files: `%{_cross_unitdir}/myservice.service`

### Adding a Template

1. Add source: `Source5: mytemplate-conf`
2. Install: `install -D -m 0644 %{S:5} %{buildroot}%{_cross_templatedir}/mytemplate-conf`
3. Add to %files: `%{_cross_templatedir}/mytemplate-conf`

### Adding a Drop-in Directory

1. Create dir: `install -d %{buildroot}%{_cross_unitdir}/myservice.service.d`
2. Add to %files: `%dir %{_cross_unitdir}/myservice.service.d`
