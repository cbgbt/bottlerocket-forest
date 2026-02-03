# Bottlerocket Packaging Guide - Updated with Prohibitive Rules

This guide summarizes key lessons and best practices for Bottlerocket packaging derived from PR reviews and formal guidelines. Rules have been updated to use prohibitive directives where they improve LLM compliance while maintaining positive rules for additional nuance.

## 1. Package Definition and Metadata

### Package Naming and Structure
- [MUST] Create spec files in `packages/[package-name]/` directory
- [MUST] Begin package names with `%{_cross_os}` prefix for cross-compilation
- [MUST] Include version, release (1%{?dist}), and epoch information
- [MUST] Provide concise summary and accurate license information using SPDX identifiers
- [MUST] Use `%{summary}.` for subpackage descriptions (do not rewrite)
- [MUST] Prefix shared library package names with `lib` (e.g., `libjson-c`)

### Package Descriptions
- [MUST] Begin package descriptions with `%{_cross_os}` prefix
- [MUST] Maintain consistent description formatting
- [MUST] Use proper punctuation and capitalization for technologies and brands
- [MUST] Use sentence case for summaries
- [MUST] Be informative but concise
- [MUST] Use accurate terminology
- [MUST NOT] Use circular descriptions that reference the package name
- [MUST NOT] Create ambiguous support claims
- [MUST] Ensure descriptions are complete
- [SHOULD] Keep descriptions concise and informative
- [SHOULD] Focus on functionality rather than repeating package name
- [SHOULD] Use official project descriptions when available

## 2. Source Management and Versioning

### Source Organization
- [MUST] Group source files by numbered ranges:
  - < 100: miscellaneous files
  - 1xx: systemd units
  - 2xx: tmpfiles configurations
  - 3xx: udev rules
  - 4xx: license files
- [MUST NOT] Number sources outside established ranges without documentation
- [MUST NOT] Renumber existing sources when removing items from the middle
- [MUST] Leave gaps in source numbering rather than renumbering when removing sources
- [MUST] Install files in source order matching Source declarations
- [SHOULD] Use meaningful comments to describe source files

### Version Management
- [MUST] Use major version specifications (e.g., `GO_MAJOR="1.25"` not `GO_VERSION="1.25.4"`)
- [MUST] Document version-specific patches
- [MUST] Pin to specific Git commit hashes rather than branch names
- [MUST] Consider compatibility guarantees from semantic versioning
- [MUST] Document version constraints and rationale for version choices
- [MUST] Prioritize reducing maintenance overhead
- [MUST] Consider maintenance implications of version specifications
- [MUST] Balance precision with practicality
- [MUST NOT] Use release versions that don't match actual source artifact versions
- [MUST NOT] Use beta/alpha designations in Release field when packaging stable upstream versions
- [MUST NOT] Pre-initialize environment variables at the top of scripts as this will override inherited values
- [SHOULD] Allow automatic patch version updates within major version series
- [SHOULD] Use consistent version variables (e.g., `%global gover` for Go)
- [SHOULD] Avoid "busywork commits" from minor version bumps
- [SHOULD] Follow consistent versioning patterns
- [SHOULD] Make spec files resilient to minor version changes
- [SHOULD] Group related version variables

### Patch Management
- [MUST] Number patches in logical ranges (1000+ for general patches)
- [MUST] Document the purpose of each patch
- [MUST] Maintain traceability for patches, preferably using `git cherry-pick -x`
- [MUST] Align patches with Bottlerocket's filesystem layout
- [MUST] Document upstream issue references
- [MUST] Use `-p1` instead of `-p0001` for patch strip levels
- [MUST] Avoid confusion with patch naming conventions
- [MUST] Follow standard CLI argument conventions
- [MUST] Maintain clear distinction between patch names and strip levels
- [MUST] Use consistent patch strip level conventions
- [SHOULD] Group related patches in distinct number ranges
- [SHOULD] Adapt upstream software to Bottlerocket conventions
- [SHOULD] Minimize invasive patches
- [SHOULD] Follow conventional RPM packaging practices

## 3. License Management

### License Identification
- [MUST] Use current SPDX format for license identifiers (e.g., `LGPL-2.1-only` not `LGPL-2.1`)
- [MUST NOT] Accept license suggestions without running `askalono` verification
- [MUST NOT] Change SPDX operators without understanding semantic implications
- [MUST] Include all licenses in the `License` field using SPDX format
- [MUST] Include all license files in the `%license` section of the spec file
- [MUST] Install license files with 0644 permissions to `%{_cross_licensedir}`
- [MUST] Follow source numbering conventions (4xx range for license files)
- [MUST] Include all licenses present in dependencies and third-party code
- [MUST NOT] Use wildcard patterns like `COPYING.*` in %license directives
- [MUST] Verify license accuracy for all components
- [MUST] Check for third-party licenses in files like THIRD-PARTY-LICENSES.txt
- [MUST] Update license identifiers during package maintenance
- [SHOULD] Use clean paths for license files without leading `./`

### Attribution Requirements
- [MUST] Include `%{_cross_attribution_file}` in the main package `%files` section for all packages
- [MUST] Use `%cross_generate_sbom` to generate SBOM files for all packages at build time
- [MUST] Use `%cross_scan_attribution` to generate attribution files for Go and Rust packages (for their dependencies)
- [MUST] Include `%{_cross_attribution_vendor_dir}` in the main package `%files` section for Go and Rust packages
- [MUST] Use `--clarify` option with `%cross_scan_attribution` when clarify.toml is available
- [MUST] Install SBOM files to `%{_cross_sbom_package_dir}` for all packages

### Complex License Expressions
- [MUST] Use proper SPDX operators (`AND`, `OR`) for complex licensing scenarios
- [MUST] Consider specific usage context when determining applicable license
- [MUST] Pay special attention to dual-licensing conditions
- [MUST] Handle kernel module licensing correctly
- [MUST] Understand SPDX operator semantics:
  - "OR" means choice of license (dual-licensed)
  - "AND" means both licenses apply (different parts have different licenses)
- [MUST NOT] Use complex license expressions without clear documentation of rationale
- [MUST NOT] Create logically redundant SPDX expressions (e.g., `MIT OR (MIT AND GPLv2)`)
- [MUST] Verify license compatibility with Bottlerocket requirements
- [MUST] Update license expressions when usage changes
- [SHOULD] Document license reasoning for non-obvious decisions

## 4. Build Configuration and Compilation

### General Build Settings
- [MUST] Explicitly disable optional features unless absolutely necessary
- [MUST] Disable generation of metadata files (man pages, translations, etc.)
- [MUST NOT] Override default compiler flags unless strictly required
- [MUST] Enable SELinux support if the package supports it
- [MUST] Remove static libraries unless there's a documented need
- [MUST NOT] Build static libraries without documented justification
- [MUST NOT] Use post-build file exclusion (`%exclude`) as a substitute for proper build configuration
- [MUST NOT] Disable debug package creation without compelling technical justification
- [MUST] Verify GPG signatures for source packages when available
- [MUST] Set appropriate build flags for cross-compilation
- [MUST] Avoid unnecessary package-specific build configurations
- [MUST] Remove redundant configurations that duplicate system-wide settings
- [MUST] Document exceptions when package-specific configurations are necessary
- [SHOULD] Use `--disable-static` when configuring packages
- [SHOULD] Disable features at build time rather than excluding files afterward
- [SHOULD] Prefer system defaults for build configuration

### Build System Specifics
- [MUST NOT] Omit `-DCMAKE_INSTALL_PREFIX:PATH=%{_cross_prefix}` in CMake builds
- [MUST] Use `%cross_configure` for autotools-based packages
- [MUST] Use `%set_cross_go_flags` for Go builds
- [MUST] Configure CMake to respect cross-compilation environment
- [MUST] Verify autotools configuration output
- [MUST NOT] Combine configuration and build steps in unconventional ways
- [MUST NOT] Move build logic to `%prep` section as it violates RPM conventions
- [SHOULD] Use `%{cross_cmake}` for CMake-based packages
- [SHOULD] Disable unnecessary autotools features
- [SHOULD] Separate `%cross_cmake` and `cmake --build` steps

### Reproducible Builds
- [MUST] Create reproducible archives across different build environments
- [MUST] Use sorted file names (`--sort=name`)
- [MUST] Use fixed ownership (`--owner=0 --group=0 --numeric-owner`)
- [MUST] Document build steps clearly
- [MUST] Remove environment-specific information
- [MUST] Avoid embedding build-time information (timestamps, hostnames)
- [MUST] Use standardized tar parameters
- [MUST] Control file modification times consistently
- [MUST] Use consistent build flags across related packages
- [MUST] Ensure builds are deterministic across environments
- [SHOULD] Set fixed timestamps using `source_date_epoch`
- [SHOULD] Use deterministic build flags
- [SHOULD] Reference reproducible build standards

### Dependencies
- [MUST NOT] Include dependencies on scripting languages (Python, Bash, Ruby, etc.)
- [MUST NOT] Use OpenSSL dependencies; use Bottlerocket's `libcrypto` instead
- [SHOULD] Verify the package is actively maintained and well-funded before inclusion

## 5. File Installation and Directory Structure

### Directory Structure
- [MUST] Use cross-compilation macros (`%{_cross_bindir}`, `%{_cross_libdir}`, etc.) instead of hardcoded paths
- [MUST NOT] Install files at `/etc` or `/var`
- [MUST] Use `%{_cross_tmpfilesdir}` for tmpfiles.d configurations
- [MUST] Use `%{_cross_unitdir}` for systemd unit files
- [MUST] Use `%{_cross_templatedir}` for templates
- [MUST] Use `%{_cross_factorydir}%{_cross_sysconfdir}/[path]` for factory defaults
- [MUST] Use `%{_cross_datadir}` for data files
- [MUST] Use versioned directories to prevent collisions between different driver/library versions
- [MUST] Follow consistent directory naming patterns
- [MUST] Use standard directory hierarchy
- [MUST] Use `%dir` directive for directories owned by the package
- [MUST] Maintain parallel directory structures for variants (like FIPS)
- [SHOULD] Group related path definitions for clarity
- [SHOULD] Create logical subdirectories based on function

### File Organization and Permissions
- [MUST] Install binaries to `%{_cross_bindir}` rather than `%{_cross_sbindir}` when possible
- [MUST] Use consistent permissions (0644 for config files, 0755 for executables)
- [MUST] Group files by functionality in the `%files` section
- [MUST NOT] Scatter related files across multiple unrelated sections in %files
- [MUST NOT] Install pkg-config files to `/usr/share/pkgconfig/` instead of `/usr/lib/pkgconfig/`
- [MUST] Use `%exclude` for files that should not be included
- [MUST] Exclude metadata files (man pages, translations, etc.)
- [MUST] Exclude specific directories if generated:
  - `%{_cross_infodir}`
  - `%{_cross_localedir}`
  - `%{_cross_mandir}`
  - `%{_cross_datadir}/doc/*`
  - `%{_cross_datadir}/locale/*`
- [SHOULD] Use relative symlinks instead of absolute paths
- [SHOULD] List directories first, followed by files within those directories

### Symlink Management
- [MUST] Keep symlinks and target binaries in the same package
- [MUST NOT] Create symlinks across package boundaries
- [MUST NOT] Create absolute symlinks when relative paths would work
- [MUST] Avoid implicit dependencies through symlinks
- [MUST] Use relative symlinks (`ln -rs`)
- [MUST] Maintain security domain boundaries
- [MUST] Create symlinks from the correct source
- [MUST] Ensure proper dependency tracking
- [MUST] Design packages to prevent broken references
- [SHOULD] Maintain clear package boundaries
- [SHOULD] Use subpackages for organization
- [SHOULD] Follow established patterns
- [SHOULD] Document any exceptions for cross-package symlinking

## 6. Package Architecture and Subpackages

### Subpackage Structure
- [MUST NOT] Place development files (headers, unversioned .so files) in main packages
- [MUST] Ensure `-devel` packages require their base package using `Requires: %{name}`
- [MUST] Place unversioned symlinks (`.so`) in `-devel` packages
- [MUST] Place versioned libraries (`.so.*`) in main packages
- [MUST] Create separate `-bin` and `-fips-bin` packages for binaries requiring FIPS compliance
- [MUST] Use conditional requirements for variant-specific features
- [MUST NOT] Use ambiguous or inconsistent package naming (avoid `-utils`, `-misc`, `-common`)
- [MUST] Use consistent subpackage suffixes (e.g., `-bin`, `-fips-bin`, `-devel`, `-extras`)
- [SHOULD] Separate binaries into their own packages when appropriate
- [SHOULD] Prefer full words over abbreviations in package names
- [SHOULD] Group related functionality in logical subpackages

### Package Organization
- [MUST] Create capability hierarchies where main packages depend on capability-providing subpackages
- [MUST] Use descriptive capability names indicating functionality (e.g., `hostname-imds(binaries)`)
- [MUST] Follow consistent organization patterns
- [MUST] Install plugins to appropriate directories
- [MUST] Maintain clean, focused sections
- [SHOULD] Group files by functionality
- [SHOULD] Maintain clean, non-redundant spec files

## 7. Dependency Management and Relationships

### Virtual Capabilities and Package Relationships
- [MUST NOT] Require the same capability a package provides
- [MUST] Use `%{name}` for self-reference in dependencies, not hardcoded package names
- [MUST] Use `package(capability)` syntax with `%{_cross_os}` prefix for virtual capabilities
- [MUST] Use conditional requirements with `image-feature(fips)` and `image-feature(no-fips)`
- [MUST] Ensure binary packages require parent packages with conditional requirements
- [MUST] Declare conflicts between mutually exclusive packages (FIPS vs non-FIPS)
- [MUST] List build dependencies with `BuildRequires:` using `%{_cross_os}` prefix
- [MUST] List runtime dependencies with `Requires:` directive
- [MUST NOT] Add virtual capability provides for backwards compatibility when no previous package exists
- [MUST NOT] Use version-specific conditional dependencies when virtual capabilities would work
- [SHOULD] Review dependency chains carefully to prevent circular references
- [SHOULD] Review dependency completeness across related packages

### Dependency Organization
- [MUST] Organize dependencies alphabetically
- [MUST] Group dependencies by type
- [MUST] Follow established patterns

## 8. FIPS Compliance and Security Variants

### FIPS Package Structure
- [MUST] Create separate `-bin` and `-fips-bin` packages for binaries requiring FIPS compliance
- [MUST] Create FIPS-specific packages for Go packages only if they depend on Go crypto libraries
- [MUST] Use conditional requirements with image features
- [MUST] Provide virtual capabilities from both variants (e.g., `%{_cross_os}logdog(binaries)`)
- [MUST] Declare conflicts between variants to prevent simultaneous installation
- [MUST] Use `%{_cross_libexecdir}` for standard binaries and `%{_cross_fips_libexecdir}` for FIPS-compliant binaries
- [MUST] Create self-contained symlinks within respective directory trees
- [MUST] Apply FIPS compliance to all security-sensitive components
- [MUST] Follow standard package structure pattern (main package + variant binaries)
- [MUST] Use `Provides: %{name}(binary)` pattern for both variants
- [MUST] Use conditional requirements with parent packages
- [MUST NOT] Exclude hardware components from FIPS variants without technical analysis
- [MUST NOT] Use blanket FIPS exclusions without evaluating cryptographic interface impact

### FIPS Implementation
- [MUST] Use `gofips build` for FIPS-compliant Go binaries
- [MUST] Maintain consistent naming conventions (`-fips-bin` suffix)
- [MUST] Use appropriate build flags for FIPS compliance
- [MUST] Maintain strict separation between FIPS and non-FIPS components
- [MUST] Keep symlinks within their security domain (no cross-domain symlinks)
- [MUST] Test symlink resolution in both environments
- [MUST] Maintain parallel symlink structures
- [SHOULD] Document FIPS compliance requirements
- [SHOULD] Consider overlay mount implications for symlinks
- [SHOULD] Document symlink relationships

## 9. Language-Specific Packaging

### Go Package Structure
- [MUST] Use standardized Go project macros (`%global goproject`, `%global gorepo`, `%global goimport`)
- [MUST] Follow consistent version variable patterns (`%global gover`, `%global rpmver`)
- [MUST] Use `%cross_go_setup` and `%cross_go_configure` for proper cross-compilation
- [MUST] Set appropriate linker flags for version information
- [MUST] Use `%set_cross_go_flags` for proper Go build flags
- [MUST] Dynamically generate compiler variables for architecture-specific compilation
- [MUST] Use structured variable naming with consistent prefixes
- [MUST] Document import paths clearly
- [MUST] Remove unused macros
- [MUST] Define version and Git revision separately
- [MUST] Use consistent build flags across related packages
- [MUST] Use `gofips build` for FIPS-compliant Go binaries
- [MUST] Use `%cross_generate_sbom` to generate SBOM files for all packages at build time
- [MUST] Use `%cross_scan_attribution` to generate attribution files for Go packages (for their dependencies)
- [MUST] Include `%{_cross_attribution_vendor_dir}` in the main package `%files` section
- [MUST] Use `--clarify` option with `%cross_scan_attribution` when clarify.toml is available
- [SHOULD] Use `-mod=vendor` for reproducible builds
- [SHOULD] Pin to specific Git commit hashes
- [SHOULD] Use major Go versions to reduce maintenance
- [SHOULD] Follow consistent organization patterns
- [SHOULD] Use package-specific prefixes for macros

### Go Toolchain Versioning
- [MUST] Maintain version consistency across build stages
- [MUST] Use major version specifications
- [MUST] Configure build environment explicitly
- [MUST] Document environment variable placement
- [MUST] Consider codegen requirements
- [MUST] Use cross-compilation macros consistently
- [SHOULD] Control the build environment through explicit variables

### Rust Package Structure
- [MUST] Use `%cross_generate_sbom` to generate SBOM files for all packages at build time
- [MUST] Use `%cross_scan_attribution` to generate attribution files for Rust packages (for their dependencies)
- [MUST] Include `%{_cross_attribution_vendor_dir}` in the main package `%files` section
- [MUST] Use `--clarify` option with `%cross_scan_attribution` when clarify.toml is available
- [SHOULD] Use cargo vendor for reproducible builds
- [SHOULD] Follow consistent organization patterns

### Kernel Module Packaging
- [MUST] Use standardized kernel source locations
- [MUST] Dynamically determine kernel version
- [MUST] Use cross-compilation macros for module directories
- [MUST] Maintain clean, non-repetitive spec files
- [MUST] Use version-specific directories for kernel module headers
- [MUST] Maintain backward compatibility
- [MUST] Consider ABI stability between kernel versions
- [MUST] Ensure consistent header paths
- [MUST] Understand implications of header placement
- [MUST] Follow vendor-specific build requirements
- [MUST] Ensure module compatibility with kernel version
- [MUST] Include proper module loading configuration
- [SHOULD] Adapt upstream packaging approaches
- [SHOULD] Document adaptations from upstream
- [SHOULD] Document header placement decisions
- [SHOULD] Balance standardization with compatibility
- [SHOULD] Test header usability
- [SHOULD] Separate open and closed components
- [SHOULD] Document vendor-specific considerations
- [SHOULD] Consider kernel ABI stability concerns
- [SHOULD] Test modules with target kernel versions

## 10. Service and Runtime Configuration

### Systemd Service Management
- [MUST] Place systemd unit files in `%{_cross_unitdir}`
- [MUST] Group service files by functionality
- [MUST] Include all necessary service files
- [MUST] Use a modular approach to service management
- [MUST] Follow a consistent service activation pattern
- [MUST] Maintain service naming consistency
- [MUST] Follow systemd best practices
- [MUST] Consider service relationships and dependencies
- [SHOULD] Follow a logical naming pattern
- [SHOULD] Maintain parallel service structures
- [SHOULD] Establish clear service dependencies
- [SHOULD] Consider service relationships
- [SHOULD] Use drop-in configurations for service customization
- [SHOULD] Use appropriate service types and restart policies

### Configuration Files
- [MUST] Minimize default configuration files
- [MUST] Evaluate necessity of default configs
- [MUST] Place default configuration files in appropriate locations
- [MUST] Follow source numbering conventions
- [MUST] Use `%{_cross_factorydir}%{_cross_sysconfdir}` for factory defaults
- [MUST] Document configuration options
- [MUST] Align configuration paths with Bottlerocket conventions
- [MUST] Understand Bottlerocket's immutable design
- [MUST] Prefer `/usr/share` over `/etc` when possible
- [MUST] Use factory defaults pattern for `/etc` files
- [MUST] Verify application behavior before changing file locations
- [MUST] Focus on minimal, secure implementations
- [MUST] Use tmpfs for /etc with runtime-generated configurations
- [SHOULD] Provide sensible default configurations
- [SHOULD] Make configuration paths configurable
- [SHOULD] Prefer application's built-in defaults
- [SHOULD] Investigate application configuration options
- [SHOULD] Configure applications via build flags
- [SHOULD] Document configuration location decisions

### Tmpfiles Configuration
- [MUST] Use tmpfiles.d for runtime directory creation
- [MUST] Place tmpfiles configurations in `%{_cross_tmpfilesdir}`
- [MUST] Follow source numbering conventions (200-range)
- [MUST] Be aware of tmpfiles lexicographical ordering behavior
- [MUST] Consider naming implications for downstream customizations
- [MUST] Understand this behavior differs from other systemd components
- [MUST] Test tmpfiles configurations with potential downstream customizations
- [MUST] Choose tmpfiles names strategically
- [SHOULD] Use tmpfiles for log directories
- [SHOULD] Make configuration file paths configurable
- [SHOULD] Consider alternative approaches for configuration customization
- [SHOULD] Document tmpfiles naming considerations

### Build Environment Variables
- [MUST] Document environment variable usage
- [MUST] Verify that environment variables are effective
- [MUST] Understand application-specific configuration mechanisms
- [SHOULD] Use environment variables to control configuration paths
- [SHOULD] Investigate application-specific configuration options
- [SHOULD] Consider using variables like `SOMETHINGDIR='%{_cross_datadir}'`
- [SHOULD] Prefer build-time configuration over runtime file movement
- [SHOULD] Look for upstream documentation on configuration options

## 11. Code Quality and Style Standards

### Formatting and Style
- [MUST] Maintain precise formatting
- [MUST] Follow consistent style
- [MUST] Avoid unnecessary whitespace
- [MUST] Ensure clean, readable spec files
- [MUST] Use consistent formatting and avoid unnecessary whitespace
- [MUST NOT] Include debugging commands (like `ls -la`) in production spec files
- [MUST] Follow consistent indentation and spacing
- [MUST NOT] Use `Release: 1.%{?dist}` format with a dot separator
- [MUST NOT] Use `%{dist}` instead of `%{?dist}` for proper conditional expansion
- [MUST NOT] Include unnecessary `./` prefix in file paths
- [SHOULD] Use consistent spacing
- [SHOULD] Use meaningful variable names and avoid abbreviations

### Comments and Documentation
- [SHOULD] Add comments only when they provide context not clear from the spec file
- [MUST] Exclude comments that don't clarify the packaging decisions
- [SHOULD] Use meaningful comments to describe source files
- [MUST] Avoid redundant comments
- [SHOULD] Place comments near source definitions
- [SHOULD NOT] Add explanatory comments in the `%files` section
- [SHOULD] Prefer comments in `%build` or `%install` sections

#### Examples
✓ Useful comments that explain packaging decisions:
```spec
# Using custom build flags because upstream defaults don't work with cross-compilation
%configure --disable-shared --enable-static

# Patch needed until upstream fixes issue #1234
Patch1001: fix-cross-compilation.patch
```

✗ Useless comments that restate what's obvious:
```spec
# Install the binary
install -m 0755 %{name} %{buildroot}%{_cross_bindir}/%{name}

# Create directory
mkdir -p %{buildroot}%{_cross_datadir}
```

### File Organization in Spec Files
- [MUST] Use `%dir` directive for directories
- [MUST] Avoid redundant comments
- [MUST] Include license files with `%license` directive
- [MUST] Be explicit about directory ownership
- [MUST] Follow consistent organization patterns
- [MUST] Install files in source order
- [MUST] Avoid redundant comments across sections
- [MUST] Keep the `%files` section clean
- [MUST] Use strategic comment placement
- [MUST] Maintain clean, focused sections
- [SHOULD] Group files by functionality
- [SHOULD] List directories first
- [SHOULD NOT] Add explanatory comments in the `%files` section
- [SHOULD] Prefer comments in `%build` or `%install` sections

### Spec File Consistency
- [MUST] Maintain consistency between related spec files
- [MUST] Address "nit" comments about consistency
- [MUST] Align service file handling
- [SHOULD] Document intentional differences
- [SHOULD] Use consistent file organization
- [SHOULD] Review for unintentional divergence

### Shell Script Best Practices
- [MUST] Implement validation checks for external data
- [MUST] Use proper directory stack management (`pushd`/`popd`)
- [MUST] Clean up build artifacts
- [MUST] Extract and manage metadata
- [MUST] Use architecture-specific macros
- [MUST] Test scripts on all supported architectures
- [MUST] Apply quality standards to build scripts
- [MUST] Document complex operations
- [MUST NOT] Use unnecessary `cat` commands when tools can read files directly
- [MUST NOT] Use user input directly in commands without validation
- [SHOULD] Follow shell scripting best practices
- [SHOULD] Separate concerns between components
- [SHOULD] Consider architecture differences
- [SHOULD] Use conditional logic for architecture-specific operations
- [SHOULD] Refactor repetitive operations

## 12. Testing and Quality Assurance

### Quality Assurance
- [MUST] Test packages for both x86_64 and aarch64 architectures
- [MUST] Verify all dependencies are satisfied for both architectures
- [MUST] Ensure no files installed in `/etc` or `/var` unless necessary
- [MUST] Verify no scripting language dependencies are introduced
- [MUST] Check all licenses are properly documented
- [MUST] Document any intentional rpmlint warnings
- [SHOULD] Run equivalent checks to Fedora's rpmlint
- [MAY] Ignore rpmlint errors related to cross-compilation macros

### Kubernetes Component Testing
- [MUST] Use version-specific packages for Kubernetes components
- [MUST] Maintain version-specific dependencies
- [MUST] Use version suffixes consistently
- [MUST] Consider upgrade paths
- [SHOULD] Allow multiple versions to coexist in the repository
- [SHOULD] Document version compatibility requirements
- [SHOULD] Maintain parallel structures across versions

### Component Relationships Testing
- [MUST] Understand component relationships
- [MUST] Package credential providers appropriately
- [MUST] Maintain version alignment
- [MUST] Separate components logically
- [MUST] Use capability-based dependencies when possible
- [MUST] Consider actual technical relationships for dependencies
- [MUST] Maintain component independence when appropriate
- [MUST] Reflect actual technical requirements in dependencies
- [MUST] Balance minimalism with practical usability
- [SHOULD] Consider cloud-specific integration needs
- [SHOULD] Avoid unnecessary dependencies
- [SHOULD] Group related components logically
- [SHOULD] Use transitive dependencies when appropriate
- [SHOULD] Consolidate license files for tightly coupled components
- [SHOULD] Consider specialized deployment scenarios
- [SHOULD] Document dependency decisions

### Feature Capability Testing
- [MUST] Ensure feature advertisement matches actual capabilities
- [MUST] Document feature limitations
- [MUST] Consider runtime failures vs. clean rejections
- [MUST] Avoid misleading capability advertisement
- [SHOULD] Consider integration implications
- [SHOULD] Test feature compatibility

### Plugin Integration Testing
- [MUST] Understand plugin discovery mechanisms
- [MUST] Consider version compatibility
- [MUST] Package CNI plugins consistently
- [MUST] Install plugins to standard locations
- [MUST] Document plugin functionality
- [MUST] Consider security implications
- [SHOULD] Test plugin integration
- [SHOULD] Document plugin capabilities
- [SHOULD] Group related plugins
- [SHOULD] Consider plugin dependencies
- [SHOULD] Test plugin functionality

### Cloud Provider Integration Testing
- [MUST] Package cloud-specific components appropriately
- [MUST] Maintain cloud provider compatibility
- [MUST] Test cloud integration functionality
- [MUST] Follow cloud provider best practices
- [SHOULD] Consider multi-cloud support
- [SHOULD] Document cloud provider requirements
- [SHOULD] Consider security implications of cloud integration

## 13. Security and Permissions

### Security Best Practices
- [MUST] Protect sensitive data using SELinux labels (secret_t)
- [MUST] Consider FIPS compliance requirements for cryptographic components
- [MUST NOT] Include unnecessary components
- [MUST] Use appropriate file permissions
- [SHOULD] Avoid hardcoded paths that could create security vulnerabilities
- [SHOULD] Use relative symlinks rather than absolute paths

### File Permissions
- [MUST] Use consistent permissions (0644 for config files, 0755 for executables)
- [MUST] Install license files with 0644 permissions to `%{_cross_licensedir}`
- [MUST] Protect sensitive data using SELinux labels (secret_t)

## 14. Maintenance and Long-term Support

### Maintenance Practices
- [MUST] Update all dependent packages when moving components
- [MUST] Maintain backward compatibility unless breaking changes necessary
- [MUST] Document workarounds with references to upstream issues
- [MUST] Document version-specific patches and their purposes
- [MUST] Verify GPG signatures for source packages when available
- [MUST NOT] Accept reviewer suggestions without verification when they affect core functionality
- [MUST NOT] Add unnecessary maintenance overhead by tracking upstream hardware support timelines
- [MUST NOT] Use predictive documentation about vendor roadmaps
- [MUST NOT] Create speculative comments about when hardware support might be added
- [SHOULD] Minimize maintenance overhead with flexible version specifications
- [SHOULD] Consider downstream impact when making changes
- [SHOULD] Document rationale for packaging decisions
- [SHOULD] Make spec files resilient to minor version changes

### Long-term Considerations
- [MUST] Consider package evolution
- [MUST] Design packages to prevent broken references
- [MUST] Maintain clear ownership boundaries
- [MUST] Consider long-term maintainability
- [SHOULD] Anticipate future changes
- [SHOULD] Document any exceptions for cross-package symlinking
- [SHOULD] Consider maintenance implications of cross-package symlinks
