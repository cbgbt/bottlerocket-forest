---
name: analyze-kubernetes-release
description: Analyze a Kubernetes release for Bottlerocket compatibility and required changes
---

# Analyze Kubernetes Release

Analyze a new Kubernetes release to determine compatibility with Bottlerocket and identify required changes.

## When to Use

- New Kubernetes minor version released (e.g., 1.32)
- Planning to add support for a new Kubernetes version
- Evaluating breaking changes before implementation

## Prerequisites

- Internet access for Kubernetes release notes
- Understanding of Bottlerocket's Kubernetes components (kubelet, kube-proxy, credential provider)

## Procedure

### 1. Initialize Analysis Document

Set the version being analyzed:
```bash
K8S_VERSION="1.32"  # e.g., "1.32" for Kubernetes 1.32
```

Create `$FOREST_ROOT/planning/k8s-${K8S_VERSION}-analysis.md` with this template:

```markdown
# Kubernetes ${K8S_VERSION} Analysis for Bottlerocket

**Date:** YYYY-MM-DD
**Analyst:** 
**Status:** In Progress

## Summary

Brief overview of findings.

## Breaking Changes

### Kubelet (SIG Node)

| Change | PR/KEP | Citation |
|--------|--------|----------|
| None identified | | |

### kube-proxy (SIG Network)

| Change | PR/KEP | Citation |
|--------|--------|----------|
| None identified | | |

## Deprecations

| Feature | Timeline | Citation | Bottlerocket Impact |
|---------|----------|----------|---------------------|
| None identified | | | |

## KEP Analysis

### KEP-XXXX: Title

**Stage:** alpha/beta/GA
**Impact:** High/Medium/Low
**Summary:** {cited from KEP}
**Citation:** {KEP URL or file path}
**Action:** 

## Dependency Updates

| Component | Version | Citation | Notes |
|-----------|---------|----------|-------|
| Go | | | |
| containerd API | | | |

## Go/No-Go

**Recommendation:** GO / NO-GO / CONDITIONAL
**Rationale:** {based on cited facts above}
**Blockers:** {cite specific changes}
```

### 2. Download CHANGELOG

```bash
curl -sL "https://raw.githubusercontent.com/kubernetes/kubernetes/master/CHANGELOG/CHANGELOG-${K8S_VERSION}.md" > /tmp/k8s-${K8S_VERSION}-changelog.md
```

### 3. Analyze Changes Using fact-find

**For each category, use fact-find to get cited answers:**

```
USING SKILL "fact-find"

Question: "What are the Urgent Upgrade Notes in Kubernetes {VERSION} CHANGELOG?"
Question: "What SIG Node changes affect kubelet in Kubernetes {VERSION}?"
Question: "What SIG Network changes affect kube-proxy in Kubernetes {VERSION}?"
Question: "What credential provider changes are in Kubernetes {VERSION}?"
```

**Document each finding with its citation** in the analysis document.

### 4. KEP Deep Dive (If Breaking Changes Found)

For breaking changes identified in step 3, use fact-find to analyze the KEP:

```
USING SKILL "fact-find"

Question: "What does KEP-XXXX change about kubelet behavior?"
Question: "What configuration changes does KEP-XXXX require?"
```

### 5. Check EKS-D Status

```bash
curl -s "https://api.github.com/repos/aws/eks-distro/releases" | \
    grep -o '"tag_name": "v1-'${K8S_VERSION//./}-'[^"]*"' | head -5
```

### 6. Make Go/No-Go Decision

**Base decision on cited facts from steps 3-4.**

**GO:** No blockers identified in cited changes
- No breaking kubelet/kube-proxy changes found, OR
- Breaking changes have simple fixes (cite specific changes)

**NO-GO:** Blockers exist in cited changes
- Kubelet flag removal requiring template changes (cite KEP/PR)
- Config format changes (cite KEP/PR)
- Dependency issues (cite version requirements)

**CONDITIONAL:** Minor issues in cited changes
- Deprecation warnings (cite specific deprecations)
- Optional features can be disabled (cite feature gates)

**⚠️ All recommendations must reference specific citations from the analysis.**

## Validation

- [ ] Analysis document created
- [ ] All findings have citations (KEP URLs, PR numbers, CHANGELOG sections)
- [ ] fact-find used for each analysis question
- [ ] Go/No-Go decision references specific cited facts
- [ ] No unsupported claims or synthesis

## Common Issues

**Too many results:** Focus on "Breaking" and "Deprecation" in change descriptions.

**Unclear impact:** Use fact-find to ask "Does {change} affect kubelet configuration?"

**Missing citations:** Every claim in the analysis must trace to a source.

## Reference

- [Kubernetes CHANGELOG](https://github.com/kubernetes/kubernetes/tree/master/CHANGELOG)
- [KEPs](https://github.com/kubernetes/enhancements/tree/master/keps)
- [SIG Node](https://github.com/kubernetes/community/tree/master/sig-node)
