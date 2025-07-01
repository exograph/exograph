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

## Release Process

To create a new release:

1. Run the release script: `./scripts/release.sh [major|minor|patch]`
2. This will:
   - Create a release branch
   - Update `Cargo.toml` version
   - Create and push a git tag
3. The "Build Binaries" workflow will automatically:
   - Create a draft release with generated release notes
   - Build binaries for all platforms
   - Upload binaries to the draft release
4. The "Process Release Notes" workflow will then clean up the notes by:
   - Removing author names from PR entries
   - Converting PR links to `#1234` format
   - Removing empty bullet points
   - Capitalizing titles properly
5. Review the cleaned release notes and binaries in the draft
6. Publish the release when satisfied

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