# Forester Dev Mode Feature

## Problem

When developing changes to Twoliter itself, developers need to test those changes against real kits and variants in the forest. By default, kits and variants use the released version of Twoliter (installed via their build process). We need a way to temporarily use a locally-built Twoliter from `./twoliter` in the forest.

## Solution: Dev Mode Commands

Add a `dev-mode` command group to forester that manages switching between released and local versions of forest components.

## Commands

### Enable Local Twoliter

```bash
forester dev-mode twoliter enable
```

**Behavior:**
- Build twoliter from `./twoliter` directory
- For each kit and variant directory in the forest:
  - Create/update symlink at `tools/twoliter` pointing to the locally-built binary
  - Or use whatever mechanism makes the local twoliter take precedence
- Track which directories were modified (for cleanup)
- Idempotent - safe to run multiple times

**Output:**
- Confirmation of what was modified
- Path to the local twoliter being used
- Instructions for reverting

### Disable Local Twoliter

```bash
forester dev-mode twoliter disable
```

**Behavior:**
- Remove symlinks/modifications made by enable
- Restore original state (released twoliter will be used)
- Clean up any tracking state
- Idempotent - safe to run even if not enabled

**Output:**
- Confirmation of what was reverted
- Note that released twoliter will now be used

### Check Dev Mode Status

```bash
forester dev-mode status
```

**Behavior:**
- Report whether local twoliter is enabled
- Show which directories are affected
- Show path to local twoliter if enabled
- Exit code 0 if any dev mode active, non-zero if all default

**Output:**
- Clear status of dev mode configuration
- Paths and versions where applicable

## Design Considerations

### Discovery Mechanism

Forester needs to find all directories that use Twoliter:
- Look for `Twoliter.toml` files in the forest
- Check standard locations: `kits/*/`, `bottlerocket/variants/*/`, `bottlerocket/`
- Consider caching this list or computing it each time

### Twoliter Integration Method

Options for making local twoliter take precedence:
1. **Symlink in tools/** - Create `tools/twoliter` symlink in each project
2. **PATH manipulation** - Modify shell environment (harder for agents)
3. **Twoliter.toml modification** - If twoliter supports local overrides (check docs)

Choose the approach that's most reliable and least invasive.

### State Tracking

Forester should track what it modified:
- Store state in `~/.config/forester/dev-mode.json` or similar
- Track which directories were modified and how
- Use this for cleanup during disable
- Handle case where files were manually modified

### Build Management

When enabling:
- Should forester rebuild twoliter automatically?
- Or assume it's already built and just link it?
- Consider: `forester dev-mode twoliter enable --rebuild` flag

### Safety

- Verify local twoliter exists and is executable before enabling
- Warn if local twoliter is stale (not recently built)
- Provide clear error messages if enable fails partway through
- Make disable robust even if enable didn't complete

## Future Extensions

This pattern could extend to other components:
```bash
forester dev-mode sdk enable        # Use local SDK
forester dev-mode core-kit enable   # Use local core-kit build
```

But start with just twoliter since it's the most common development scenario.

## Integration with Skills

Create a skill `develop-twoliter` that documents:
1. Making changes to twoliter code
2. Running `forester dev-mode twoliter enable`
3. Testing changes against kits/variants
4. Running `forester dev-mode twoliter disable` when done

## Implementation Notes

- Add `dev-mode` as a top-level command in forester's CLI
- Subcommands: `twoliter enable`, `twoliter disable`, `status`
- Consider using a library like `serde_json` for state tracking
- Use `std::fs` for symlink operations
- Use `std::process::Command` to build twoliter if needed
- Provide helpful error messages at each step
