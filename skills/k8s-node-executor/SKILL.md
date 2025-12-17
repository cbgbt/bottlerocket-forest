---
name: k8s-node-executor
description: Execute commands on Bottlerocket K8s nodes via a privileged pod with host namespace access
---

# Skill: K8s Node Executor

## Purpose

Deploy a privileged pod to execute commands directly on Bottlerocket nodes for debugging, testing, and exploration. Returns command output to stdout for scriptable use.

## When to Use

- Debugging node-level issues on Bottlerocket K8s nodes
- Inspecting host filesystem, processes, or network
- Running apiclient commands to view/modify Bottlerocket settings
- Container runtime inspection

## Prerequisites

- kubectl access to the K8s cluster with Bottlerocket nodes
- Permissions to create privileged pods
- Target node name (optional - defaults to any Linux node)

## Procedure

### 1. Deploy the Executor Pod

**Basic (any node):**
```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: br-executor
spec:
  hostNetwork: true
  hostPID: true
  hostIPC: true
  containers:
  - name: exec
    image: amazonlinux:2023
    securityContext:
      privileged: true
    volumeMounts:
    - name: host
      mountPath: /host
    command: ["sleep", "infinity"]
  volumes:
  - name: host
    hostPath:
      path: /
  restartPolicy: Never
  nodeSelector:
    kubernetes.io/os: linux
EOF
```

**Target specific node:**
```bash
kubectl apply -f - <<'EOF'
apiVersion: v1
kind: Pod
metadata:
  name: br-executor
spec:
  nodeName: NODE_NAME_HERE
  hostNetwork: true
  hostPID: true
  hostIPC: true
  containers:
  - name: exec
    image: amazonlinux:2023
    securityContext:
      privileged: true
    volumeMounts:
    - name: host
      mountPath: /host
    command: ["sleep", "infinity"]
  volumes:
  - name: host
    hostPath:
      path: /
  restartPolicy: Never
EOF
```

### 2. Wait for Ready

```bash
kubectl wait --for=condition=Ready pod/br-executor --timeout=60s
```

### 3. Execute Commands

```bash
# Single command
kubectl exec br-executor -- <command>

# Interactive shell
kubectl exec -it br-executor -- /bin/bash
```

### 4. Cleanup

```bash
kubectl delete pod br-executor
```

## Common Commands

### Bottlerocket Settings (apiclient)

```bash
# View all settings
kubectl exec br-executor -- /host/usr/bin/apiclient get settings

# View specific setting
kubectl exec br-executor -- /host/usr/bin/apiclient get settings.kubernetes

# View OS info
kubectl exec br-executor -- /host/usr/bin/apiclient get os

# Modify setting
kubectl exec br-executor -- /host/usr/bin/apiclient set motd="Debug session"
```

### Host Filesystem

```bash
# OS release
kubectl exec br-executor -- cat /host/etc/os-release

# Bottlerocket settings JSON
kubectl exec br-executor -- cat /host/etc/bottlerocket/settings.json

# List host binaries
kubectl exec br-executor -- ls /host/usr/bin/
```

### System Info

```bash
# Kernel version
kubectl exec br-executor -- uname -a

# Memory
kubectl exec br-executor -- free -h

# Disk
kubectl exec br-executor -- df -h

# Processes
kubectl exec br-executor -- ps aux

# Loaded modules
kubectl exec br-executor -- lsmod
```

### Networking

```bash
# Interfaces
kubectl exec br-executor -- ip addr

# Routes
kubectl exec br-executor -- ip route

# Listening ports
kubectl exec br-executor -- ss -tlnp

# iptables
kubectl exec br-executor -- iptables -L -n -v
```

### Container Runtime

```bash
# List containers (k8s namespace)
kubectl exec br-executor -- ctr -n k8s.io containers list

# List images
kubectl exec br-executor -- ctr -n k8s.io images list

# Inspect container
kubectl exec br-executor -- ctr -n k8s.io containers info <container-id>
```

### Systemd Services

```bash
# List services
kubectl exec br-executor -- chroot /host systemctl list-units --type=service

# Service status
kubectl exec br-executor -- chroot /host systemctl status kubelet
```

## Security Warning

**This pod has full node access.** It can:
- Read/modify any host file
- Access all processes and containers  
- Change system configuration
- Affect node stability

**Best practices:**
- Use only in dev/test environments
- Delete immediately after use
- Never leave running in production

## Troubleshooting

### Pod won't start

```bash
kubectl describe pod br-executor
```

Common causes:
- PodSecurityPolicy/PodSecurityStandard blocking privileged pods
- Node selector doesn't match any nodes
- Image pull failure

### Command not found

Host binaries need full path:
```bash
# Wrong
kubectl exec br-executor -- apiclient get os

# Right  
kubectl exec br-executor -- /host/usr/bin/apiclient get os
```

### Install additional tools

```bash
kubectl exec br-executor -- yum install -y tcpdump strace bind-utils
```
