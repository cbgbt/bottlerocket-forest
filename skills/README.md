# Bottlerocket Forest Skills

Claude Code skills for common Bottlerocket development workflows.

## Available Skills

- **local-registry** - Start and manage a local OCI registry for development
- **update-twoliter** - Update all repositories to a new Twoliter version

## Using Skills

Skills are automatically discovered by Claude Code when placed in this directory. Invoke them by name in conversation:

```
"Use the local-registry skill to start a registry"
"Apply the update-twoliter skill to bump to version 0.13.0"
```

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
