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

## Available Skills

- **research-with-citations** - Answer questions about Bottlerocket by finding and reading relevant documentation
- **local-registry** - Start and manage a local OCI registry for development
- **build-kit-locally** - Build a kit and publish it to a locally hosted registry for development testing
- **build-variant-from-local-kits** - Build a variant using locally published kits for development validation
- **update-twoliter** - Update all repositories to a new Twoliter version

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
