# Git Hooks

This directory contains git hooks for the Exograph project.

## Setup

To install these hooks, run from the project root:

```bash
./.githooks/setup.sh
```

Or manually create symlinks:

```bash
ln -sf ../../.githooks/pre-commit .git/hooks/pre-commit
ln -sf ../../.githooks/commit-msg .git/hooks/commit-msg
```
