---
name: k8s-node-executor
description: Execute commands on Bottlerocket K8s nodes via kubectl debug
---

# Skill: K8s Node Executor

## Purpose

Execute commands directly on Bottlerocket nodes for debugging, testing, and exploration.

## When to Use

- Debugging node-level issues on Bottlerocket K8s nodes
- Inspecting host filesystem, processes, or network
- Running apiclient commands to view/modify Bottlerocket settings
- Container runtime inspection

## Prerequisites

- kubectl access to the K8s cluster with Bottlerocket nodes
- Target node name (get via `kubectl get nodes`)

## Procedure

Use `kubectl debug` with `--profile=sysadmin` for full host access including the Bottlerocket API socket.

### Execute Commands

```bash
# Single command
kubectl debug node/<node-name> -it --image=busybox --profile=sysadmin -- <command>

# Interactive shell
kubectl debug node/<node-name> -it --image=busybox --profile=sysadmin -- /bin/sh
```

**Note:** The `--profile=sysadmin` flag is required.
Without it, apiclient commands fail with "Permission denied" on the API socket.

### Cleanup

Debug pods are automatically cleaned up when the session ends.
To manually remove:

```bash
kubectl get pods -o name | grep node-debugger | xargs kubectl delete
```

## Common Commands

Replace `<node>` with your node name.

### Bottlerocket Settings (apiclient)

```bash
# View all settings
kubectl debug node/<node> -it --image=busybox --profile=sysadmin -- chroot /host /usr/bin/apiclient get settings

# View specific setting
kubectl debug node/<node> -it --image=busybox --profile=sysadmin -- chroot /host /usr/bin/apiclient get settings.kubernetes

# View OS info
kubectl debug node/<node> -it --image=busybox --profile=sysadmin -- chroot /host /usr/bin/apiclient get os

# Modify setting
kubectl debug node/<node> -it --image=busybox --profile=sysadmin -- chroot /host /usr/bin/apiclient set motd="Debug session"
```

### Host Filesystem

```bash
# OS release
kubectl debug node/<node> -it --image=busybox --profile=sysadmin -- cat /host/etc/os-release

# Bottlerocket settings JSON
kubectl debug node/<node> -it --image=busybox --profile=sysadmin -- cat /host/etc/bottlerocket/settings.json

# List host binaries
kubectl debug node/<node> -it --image=busybox --profile=sysadmin -- ls /host/usr/bin/
```

### System Info

```bash
# Kernel version
kubectl debug node/<node> -it --image=busybox --profile=sysadmin -- uname -a

# Memory
kubectl debug node/<node> -it --image=busybox --profile=sysadmin -- free -h

# Disk
kubectl debug node/<node> -it --image=busybox --profile=sysadmin -- df -h

# Processes
kubectl debug node/<node> -it --image=busybox --profile=sysadmin -- ps aux
```

### Networking

```bash
# Interfaces
kubectl debug node/<node> -it --image=busybox --profile=sysadmin -- ip addr

# Routes
kubectl debug node/<node> -it --image=busybox --profile=sysadmin -- ip route

# Listening ports
kubectl debug node/<node> -it --image=busybox --profile=sysadmin -- ss -tlnp
```

### Container Runtime

```bash
# List containers (k8s namespace)
kubectl debug node/<node> -it --image=busybox --profile=sysadmin -- chroot /host ctr -n k8s.io containers list

# List images
kubectl debug node/<node> -it --image=busybox --profile=sysadmin -- chroot /host ctr -n k8s.io images list
```

### Systemd Services

```bash
# List services
kubectl debug node/<node> -it --image=busybox --profile=sysadmin -- chroot /host systemctl list-units --type=service

# Service status
kubectl debug node/<node> -it --image=busybox --profile=sysadmin -- chroot /host systemctl status kubelet
```

## Security Warning

**This approach grants full node access.** It can:
- Read/modify any host file
- Access all processes and containers
- Change system configuration
- Affect node stability

**Best practices:**
- Use only in dev/test environments
- Clean up immediately after use

## Troubleshooting

### Permission denied on API socket

Ensure you're using `--profile=sysadmin`.
The default profile doesn't grant socket access.

### Command not found

Host binaries need `chroot /host` prefix:
```bash
# Wrong
kubectl debug node/<node> -it --image=busybox --profile=sysadmin -- apiclient get os

# Right
kubectl debug node/<node> -it --image=busybox --profile=sysadmin -- chroot /host /usr/bin/apiclient get os
```
