# Bottlerocket Forest Skills

Skills are documented workflows for AI agents to perform common development tasks.

## Available Skills

- **local-registry** - Start and manage a local OCI registry for development

## Skill Structure

Each skill is a directory containing:
- `SKILL.md` - Main documentation with procedures and examples
- Optional helper scripts or configuration files
- Examples and test cases where applicable

## Skill Format

Skills follow this structure:

1. **Purpose** - What the skill does and why it exists
2. **When to Use** - Scenarios where this skill applies
3. **Prerequisites** - Required tools, permissions, or prior setup
4. **Procedure** - Step-by-step instructions
5. **Validation** - How to verify success
6. **Common Issues** - Known problems and solutions
7. **Related Skills** - Other skills that complement this one

## Creating New Skills

When creating a skill:
- Focus on a single, well-defined workflow
- Include concrete commands, not just descriptions
- Provide validation steps so agents can verify success
- Document failure modes you've encountered
- Link to related skills to help agents chain workflows
