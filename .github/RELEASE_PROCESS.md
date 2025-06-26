# Release Process Documentation

## Overview

This repository uses GitHub's automated release notes feature with custom categorization based on conventional commits defined in `.commitlintrc.json`.

## Commit Convention

We follow the conventional commit format: `type(scope): description`

### Commit Types and Their Categories

- `breaking:` - 🚨 Breaking Changes
- `feat:` - 🎉 Features (new features)
- `fix:` - 🐛 Bug Fixes
- `security:` - 🔒 Security
- `docs:` - 📚 Documentation
- `style:` - 🎨 Style (formatting, missing semi colons, etc)
- `refactor:` - 🏗️ Refactoring
- `perf:` - ⚡ Performance
- `test:` - 🧪 Testing
- `build:` - 🔨 Build System
- `ci:` - 👷 CI/CD
- `chore:` - 🔧 Maintenance
- `revert:` - ⏪ Reverts
- `release:` - 🏷️ Release (excluded from release notes)

## Release Notes Generation

When creating a new release on GitHub:

1. Go to the [Releases page](../../releases)
2. Click "Draft a new release"
3. Choose a tag (format: `vX.Y.Z`)
4. Click "Generate release notes"
5. GitHub will automatically:
   - Categorize commits based on PR labels
   - Group changes by category with emojis
   - Exclude dependabot changes
   - List new contributors

## PR Labeling

PRs are automatically labeled based on their title:
- Ensure PR titles follow the conventional commit format
- The `label-prs.yml` workflow will add appropriate labels
- These labels are used for release note categorization

## First-Time Contributors

The release notes will automatically highlight first-time contributors in a special section with a welcome message.

## Manual Adjustments

After generating release notes, you can:
- Edit the generated notes
- Add a summary or highlights section
- Include migration guides or breaking changes
- Add any additional context needed