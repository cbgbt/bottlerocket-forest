---
feature: 0001-multi-context-indexing
status: proposed
---

# Multi-Context Indexing

## Problem

Building Bottlerocket involves working across many repositories simultaneously.
The forest provides a unified workspace, but developers—especially AI agents—often need to work on multiple features in parallel.
Git worktrees are perfect for this: check out different branches into separate directories and let each agent work independently.

Today, sembly indexes a single directory tree.
If you create multiple worktrees, each one needs its own sembly index.
Building an index takes around ten minutes on a laptop because embedding generation is expensive.
Multiply that by several worktrees and the setup time becomes painful.
Worse, if an agent updates documentation in their worktree, they can't easily test whether their changes improve search results without rebuilding the entire index.

The forest wants to evolve toward a model where bare git clones live in a `forest-trunk/` directory and all work happens in worktrees under `worktrees/`.
But sembly's current single-context design doesn't support this workflow.

## Solution

Sembly gains the concept of a "context"—a working directory that shares a common embedding database with other contexts in the same workspace.
The key insight is that most content across worktrees is identical; only the files being actively modified differ.
By using content-addressed storage, sembly can share embeddings for unchanged content while tracking each context's unique view of the filesystem.

When you run `sembly build --context worktrees/feature-a`, sembly registers that directory as a context and indexes its files.
If the same file content exists in another context, the embedding is reused—no expensive regeneration needed.
Each context maintains its own file mapping, so searches return results appropriate to that context's state.

From an agent's perspective, nothing changes about how they use sembly.
They run searches from their worktree and get results reflecting their worktree's content.
They're unaware of other contexts, which is exactly right—agents working on different features shouldn't see each other's uncommitted changes.

## How It Works

A developer sets up the forest workspace with sembly at the root.
The database lives at `.sembly/knowledge.db` and the configuration at `.sembly.toml`, just like today.
This becomes the "default" context.

When they create a new worktree for a feature, they register it with sembly:

```bash
forester worktree create feature-auth --branch add-authentication
sembly build --context worktrees/feature-auth
```

The first index of a new worktree is fast because most files are unchanged from the default context.
Sembly hashes each file, finds the existing chunks and embeddings, and simply records that this context contains those files.
Only genuinely new or modified content requires embedding generation.

As the agent works, they might update documentation.
Running `sembly update` from within the worktree refreshes just the changed files.
The agent can immediately search to verify their documentation improvements appear in results.

Meanwhile, another agent works in a different worktree on a different feature.
Their searches return their content; they never see the first agent's uncommitted documentation changes.
The workspace supports parallel development naturally.

When a worktree is no longer needed, `sembly context remove worktrees/feature-auth` drops the context's file mappings.
The shared embeddings remain available for other contexts.
Periodically, `sembly gc` cleans up embeddings that no context references anymore.

## Benefits

The primary benefit is enabling parallel AI agent workflows without the ten-minute tax per worktree.
A new context indexes in seconds when most content is shared, making it practical to spin up worktrees freely.

Agents can iterate on documentation and immediately test search quality.
This tight feedback loop improves documentation quality—agents can verify their changes actually help before committing.

The content-addressed design also reduces storage.
Instead of duplicating embeddings across worktrees, identical content shares a single embedding.
A workspace with five worktrees uses barely more storage than one.

Finally, the design keeps sembly general-purpose.
Contexts aren't specific to the forest's worktree model—any project with multiple related directories can benefit.
A monorepo with several apps, a documentation site with versioned branches, or a research project with experimental variations all fit the model.

## Technical Notes

The database schema changes to separate content-addressed storage from per-context file mappings.
Files map to content hashes; chunks and embeddings key off content hashes.
Only the file mapping table includes context_id.

Context IDs are relative paths from the workspace root, canonicalized to avoid ambiguity.
The default context uses `.` as its ID.

Configuration remains workspace-global.
All contexts share the same `.sembly.toml`, which defines indexed directories relative to each context root.
This keeps the model simple—different config means different workspace.

The design leaves room for git-aware optimization later.
Tracking HEAD commit and dirty state per context could skip filesystem scanning entirely when nothing has changed.
The abstractions should accommodate this without requiring schema changes.

Migration from the current schema requires a rebuild.
A future migrations module (`storage/migrations/`) will provide structured schema evolution, but for this release, users simply run `sembly rebuild`.
