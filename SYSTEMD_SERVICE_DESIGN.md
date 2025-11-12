# Bottlerocket Systemd Service Design

## Overview

Bottlerocket uses a systemd service layout that differs from many common Linux distributions. Instead of booting directly to `multi-user.target`, Bottlerocket uses a staged boot process with custom synchronization targets. This ensures configuration is fully applied before workload services start, enabling features like safe A/B partition updates and consistent initialization.

## Boot Stages and Synchronization Points

The boot process flows through these key targets:

```
sysinit.target (systemd default)
    ↓
fipscheck.target (FIPS variants only)
    ↓
preconfigured.target
    ↓
configured.target
    ↓
multi-user.target (system ready)
```

Each stage must complete before the next begins. Services use `RequiredBy=` dependencies to block target completion until they finish.

## Stage 1: Early Boot (sysinit.target → fipscheck.target)

### Purpose
Basic system initialization: filesystems, devices, and FIPS validation.

### Key Services

**prepare-var.service**
- Sets up overlayfs directories for kernel modules, CNI plugins, CSI helpers
- Creates upper/lower/work directories in `/var/lib/`
- Applies SELinux labels
- `WantedBy=local-fs.target`

**FIPS Validation (FIPS variants only)**
```
check-kernel-integrity.service
    ↓
check-fips-modules.service
    ↓
fipscheck.target
    ↓
activate-preconfigured.service
```

- `check-kernel-integrity.service` - Verifies kernel integrity
- `check-fips-modules.service` - Loads and tests FIPS crypto modules (tcrypt)
- `activate-preconfigured.service` - Transitions to preconfigured.target

### When to Add Services Here
- Low-level filesystem or device setup
- Security validation that must happen before configuration
- Services with `DefaultDependencies=no`

## Stage 2: Preconfigured (preconfigured.target)

### Purpose
Initialize the datastore, start the API server, and apply system configuration.

### Service Dependency Flow

```
storewolf.service (create datastore)
    ↓
migrator.service (migrate datastore schema)
    ↓
apiserver.service (start API server)
    ↓
sundog.service (generate settings from user data/IMDS)
    ↓
settings-applier.service (render config files)
    ↓
mark-successful-boot.service (mark boot successful)
```

### Key Services

**storewolf.service**
- Creates/initializes the Bottlerocket datastore at `/var/lib/bottlerocket/datastore`
- First service in the configuration chain
- `RequiredBy=preconfigured.target`

**migrator.service**
- Migrates datastore schema between OS versions
- Reads from `/var/lib/bottlerocket-migrations`
- `Before=apiserver.service`
- `RequiredBy=preconfigured.target apiserver.service`

**apiserver.service**
- Starts the Bottlerocket API server on Unix socket
- Provides API for configuration management
- `After=storewolf.service`
- `WantedBy=preconfigured.target`

**sundog.service**
- Runs user-specified setting generators
- Can fetch settings from cloud metadata (IMDS)
- Commits settings to datastore
- `After=network-online.target apiserver.service`
- `RequiredBy=preconfigured.target`

**settings-applier.service**
- Applies settings to generate config files
- Runs `thar-be-settings --all` to render templates
- `After=storewolf.service sundog.service apiserver.service`
- `RequiredBy=preconfigured.target`
- `RefuseManualStart=true` - prevents accidental manual runs

**mark-successful-boot.service**
- Calls `signpost mark-successful-boot` for A/B partition management
- `RequiredBy=preconfigured.target`
- `RefuseManualStart=true`

### Transition to Next Stage

**activate-configured.service**
- Sets `configured.target` as default
- Starts `configured.target --no-block`
- `WantedBy=preconfigured.target`

### When to Add Services Here
- Services that need API server access
- Configuration generators or appliers
- Services that must complete before container runtime starts
- Boot validation or health checks

## Stage 3: Configured (configured.target)

### Purpose
Start the container runtime and run one-time bootstrap containers.

### Service Dependency Flow

```
preconfigured.target
    ↓
host-containerd.service (container runtime)
    ↓
bootstrap-containers@.service (one-time setup containers)
    ↓
configured.target complete
```

### Key Services

**host-containerd.service**
- Starts containerd for host containers
- Uses `/etc/host-containerd/config.toml`
- `After=network-online.target preconfigured.target`
- `WantedBy=configured.target`

**bootstrap-containers@.service** (template unit)
- Runs one-time setup containers
- Creates sentinel files at `/run/bootstrap-containers/%i.ran`
- `ConditionPathExists=!/run/bootstrap-containers/%i.ran` prevents re-runs
- `Before=configured.target`
- `After=host-containerd.service`
- `RefuseManualStart=true`

Example: `bootstrap-containers@my-setup.service` runs once, creates sentinel, never runs again.

### Transition to Next Stage

**activate-multi-user.service**
- Sets `multi-user.target` as default
- Starts `multi-user.target --no-block`
- `After=configured.target reboot-if-required.service`
- `WantedBy=configured.target`

### When to Add Services Here
- Container runtime dependencies
- One-time initialization that needs containers
- Services that prepare for workload execution

## Stage 4: Multi-User (multi-user.target)

### Purpose
System is fully operational. Start long-running services and workloads.

### Key Services

**host-containers@.service** (template unit)
- Long-running host containers (admin-container, control-container)
- `Type=simple` with `Restart=always`
- `After=host-containerd.service`
- `WantedBy=multi-user.target`

Example: `host-containers@admin.service` runs the admin container continuously.

**Workload Services** (variant-dependent)
- Kubernetes components (kubelet, etc.)
- ECS agent
- Other orchestrator agents
- `WantedBy=multi-user.target`

### When to Add Services Here
- Long-running workload services
- Services that should restart automatically
- Services that depend on full system initialization

## Complete Boot Flow Diagram

```
┌─────────────────────────────────────────────────────────────┐
│ sysinit.target                                              │
│   • prepare-var.service (filesystem setup)                  │
│   • SELinux policy loading                                  │
│   • Device initialization                                   │
└────────────────────────┬────────────────────────────────────┘
                         ↓
┌─────────────────────────────────────────────────────────────┐
│ fipscheck.target (FIPS variants only)                       │
│   • check-kernel-integrity.service                          │
│   • check-fips-modules.service                              │
│   • activate-preconfigured.service (transition)             │
└────────────────────────┬────────────────────────────────────┘
                         ↓
┌─────────────────────────────────────────────────────────────┐
│ preconfigured.target                                        │
│   ┌──────────────────────────────────────────────────────┐ │
│   │ Configuration Chain                                  │ │
│   │   storewolf → migrator → apiserver                   │ │
│   │                    ↓                                 │ │
│   │                 sundog (+ network)                   │ │
│   │                    ↓                                 │ │
│   │             settings-applier                         │ │
│   └──────────────────────────────────────────────────────┘ │
│   • mark-successful-boot.service                            │
│   • activate-configured.service (transition)                │
└────────────────────────┬────────────────────────────────────┘
                         ↓
┌─────────────────────────────────────────────────────────────┐
│ configured.target                                           │
│   • host-containerd.service                                 │
│   • bootstrap-containers@*.service (one-time)               │
│   • activate-multi-user.service (transition)                │
└────────────────────────┬────────────────────────────────────┘
                         ↓
┌─────────────────────────────────────────────────────────────┐
│ multi-user.target (System Ready)                            │
│   • host-containers@*.service (long-running)                │
│   • Workload services (kubelet, ECS agent, etc.)            │
└─────────────────────────────────────────────────────────────┘
```

## Service Patterns and Best Practices

### Blocking Target Completion

Use `RequiredBy=` to ensure a service completes before a target:

```ini
[Install]
RequiredBy=preconfigured.target
```

The target will not complete until this service finishes.

### Preventing Manual Intervention

Critical services use these directives to prevent accidental manual runs:

```ini
[Unit]
RefuseManualStart=true
RefuseManualStop=true
```

### One-Time Execution

Bootstrap containers use sentinel files:

```ini
[Unit]
ConditionPathExists=!/run/bootstrap-containers/%i.ran

[Service]
ExecStart=/usr/bin/touch /run/bootstrap-containers/%i.ran
```

### Stage Transitions

Activation services transition between stages:

```ini
[Service]
Type=oneshot
ExecStart=/usr/bin/systemctl set-default configured.target
ExecStart=/usr/bin/systemctl start configured.target --no-block
RemainAfterExit=true
```

## Debugging Boot Issues

### Check Current Target

```bash
systemctl get-default
```

### See What's Blocking a Target

```bash
systemctl list-dependencies preconfigured.target
systemctl list-jobs
```

### View Service Status

```bash
systemctl status preconfigured.target
systemctl status apiserver.service
```

### Check Service Logs

```bash
journalctl -u apiserver.service
journalctl -u preconfigured.target
```

## Adding New Services: Decision Tree

```
Does your service need to run before configuration is applied?
├─ Yes → preconfigured.target
│         RequiredBy=preconfigured.target
│
└─ No → Does it need to run once during boot?
        ├─ Yes → bootstrap-containers@.service
        │         Before=configured.target
        │
        └─ No → Does it need to run continuously?
                └─ Yes → multi-user.target
                          WantedBy=multi-user.target
```

## Key Locations

- **Service units**: `packages/release/*.service`, `packages/os/*.service`, `packages/host-ctr/*.service`
- **Target definitions**: `packages/release/*.target`
- **Datastore**: `/var/lib/bottlerocket/datastore/current`
- **Bootstrap sentinels**: `/run/bootstrap-containers/*.ran`
- **Host container configs**: `/etc/host-containers/*.env`

## Why This Design?

1. **Ordered initialization** - Each stage completes before the next begins
2. **Configuration before services** - Settings are applied before starting workloads
3. **Idempotency** - Bootstrap containers use sentinel files to run only once
4. **Isolation** - Host containers run in separate containerd instance
5. **Rollback safety** - Boot success marking enables A/B partition rollback
6. **Debuggability** - Clear stages make it easy to identify where boot fails

This staged approach ensures Bottlerocket has a consistent, predictable boot sequence where configuration is fully applied before any workload services start.
