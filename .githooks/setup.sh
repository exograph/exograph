#!/bin/bash

# Setup git hooks by creating symlinks

echo "Setting up git hooks..."

# Get the git hooks directory
GIT_HOOKS_DIR=$(git rev-parse --git-dir)/hooks

# Create symlinks for all hooks in .githooks
for hook in .githooks/*; do
    if [ -f "$hook" ] && [ ! "${hook##*/}" = "setup.sh" ] && [ ! "${hook##*/}" = "README.md" ]; then
        hook_name=$(basename "$hook")
        echo "Installing $hook_name hook..."
        ln -sf ../../.githooks/"$hook_name" "$GIT_HOOKS_DIR"/"$hook_name"
    fi
done

echo "✅ Git hooks setup complete!"
echo ""
echo "The following hooks are now active:"
echo "  - pre-commit: Runs cargo clippy and cargo fmt"
echo "  - commit-msg: Validates commit message format according to .commitlintrc.json"