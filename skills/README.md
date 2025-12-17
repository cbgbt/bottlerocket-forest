# Bottlerocket Forest Skills

Claude Code skills for common Bottlerocket development workflows.

## ⚠️ IMPORTANT: Always Use Skills When Available

**Skills are the preferred way to accomplish tasks in the forest.** They provide:
- Tested, reliable workflows
- Consistent results across sessions
- Complete procedures with validation
- Known solutions to common issues

**Before starting any task, check if a skill exists for it.** Using skills ensures quality and saves time.

## Skill Usage Protocol

When you identify that a skill should be used:

1. **Announce it clearly:**
   ```
   USING SKILL "skill-name"
   ```

2. **Load the skill file:**
   ```
   Read skills/skill-name/SKILL.md
   ```

3. **Follow the procedure exactly as documented** in the SKILL.md file

**💡 TIP: If you have todolist functionality, use it to track skill steps.** Multi-step skills benefit from explicit progress tracking.

### Delegating Skills to Subagents

When an orchestrating agent delegates a skill to a subagent:

1. **Announce the skill and provide only the question/task**
2. **Pass the SKILL.md via context_files**
3. **Do not restate or paraphrase the procedure** - trust the subagent to follow the authoritative SKILL.md

```python
# ✅ RIGHT - minimal delegation
spawn(
    '''USING SKILL "fact-find"

    QUESTION: What partition scheme does Bottlerocket use?''',
    context_files=["skills/fact-find/SKILL.md"],
    allow_tools=True
)

# ❌ WRONG - restating skill instructions
spawn(
    '''USING SKILL "fact-find"

    QUESTION: What partition scheme does Bottlerocket use?

    Use crumbly to search, read relevant files, provide citations...''',
    context_files=["skills/fact-find/SKILL.md"],
    allow_tools=True
)
```

Why: Paraphrased instructions may diverge from the SKILL.md, creating conflicting guidance.

## Available Skills

### Kubernetes Version Management

Three-phase workflow for adding new Kubernetes versions:

1. **analyze-kubernetes-release** - Deep analysis of KEPs and CHANGELOG to identify breaking changes, deprecations, and Bottlerocket impacts. Creates go/no-go recommendation.

2. **add-kubernetes-prerelease** - Package beta/rc Kubernetes sources from EKS-D pre-release artifacts. Creates new kubernetes-{version} and ecr-credential-provider-{version} packages.

3. **promote-kubernetes-ga** - Promote pre-release packages to GA by updating source URLs and hashes to official EKS-D release artifacts.

4. **add-kubernetes-variant** - Add a new Kubernetes variant to the bottlerocket repository

### Research & Documentation

- **fact-find** - Quick lookup of specific facts with citations. Use for concrete questions with definitive answers (e.g., "What partition scheme does Bottlerocket use?")
- **research-document** - Create educational documents that build understanding progressively. Use for in-depth explanations of systems or features (e.g., "Explain how Bottlerocket's update system works")

### Package Maintenance

- **update-package** - Update a kit package to the latest upstream version with checksum verification and build testing

### Build & Test

- **local-registry** - Start and manage a local OCI registry for development
- **build-kit-locally** - Build a kit and publish it to a locally hosted registry for development testing
- **build-variant-from-local-kits** - Build a variant using locally published kits for development validation
- **test-local-twoliter** - Build and test local changes to twoliter before releasing
- **update-twoliter** - Update all repositories to a new Twoliter version
- **launch-bottlerocket-ec2** - Launch Bottlerocket EC2 instances in standalone, ECS, or EKS modes
- **k8s-node-executor** - Execute commands on Bottlerocket K8s nodes via a privileged pod with host namespace access
- **ssm-executor** - Execute commands on any Bottlerocket EC2 instance via SSM (works with ECS, K8s, standalone)

### Feature Development

- **idea-honing** - Clarify feature ideas through iterative Q&A, recording insights to guide concept development
- **propose-feature-concept** - Create a new feature concept document to pitch the idea and explain the problem/solution
- **propose-feature-requirements** - Create or update feature requirements specification using EARS notation with examples and appendices
- **propose-feature-design** - Create or update feature technical design document with architecture and implementation guidance
- **propose-feature-test-plan** - Create a test plan mapping requirements and constraints to unit/integration tests
- **propose-implementation-plan** - Create an implementation plan with atomic commits that build toward a complete feature

### Settings Development

- **create-settings-model** - Define a new Bottlerocket settings model with SettingsModel trait implementation
- **add-settings-to-variant** - Wire an existing settings model into a Bottlerocket variant via settings-plugins
- **test-settings-locally** - Build and test settings SDK changes using local registry and kit builds
- **add-custom-settings** - Full workflow for adding custom settings: create model, wire to variant, test locally

## Skill Format

Each skill is a directory containing a `SKILL.md` file with:

1. **YAML frontmatter** - Metadata (name, description)
2. **Instructions** - Step-by-step procedures
3. **Optional helpers** - Scripts, templates, or resources

### SKILL.md Structure

```markdown
---
name: skill-name
description: Brief description of what the skill does
---

# Detailed instructions here
```

The frontmatter requires:
- `name`: lowercase letters, numbers, hyphens only (max 64 chars)
- `description`: concise summary (max 256 chars)

### Content Sections

- **Purpose** - What the skill does and why
- **When to Use** - Applicable scenarios
- **Prerequisites** - Required setup
- **Procedure** - Step-by-step instructions
- **Validation** - Success verification
- **Common Issues** - Known problems and solutions

## Creating New Skills

1. Create a directory with a descriptive name
2. Add `SKILL.md` with YAML frontmatter
3. Include concrete commands and examples
4. Add validation steps
5. Document common failure modes
