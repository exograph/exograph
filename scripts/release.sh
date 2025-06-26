#!/bin/sh

# Get the current banch and verify that it is main
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$CURRENT_BRANCH" != "main" ]; then
  echo "Error: You must release it from the main branch (Current branch is $CURRENT_BRANCH)." 1>&2
  exit 1
fi

# Get the current git tag, which must be in the vMAJOR.MINOR.PATCH format optionally followed by a -REV-HASH
GIT_TAG=$(git describe --tags $(git rev-list --tags --max-count=1) --match 'v*[0-9].*[0-9].*[0-9]')

# Verify that the tag is in the correct format
if ! echo $GIT_TAG | grep -Eq "^v[0-9]+\.[0-9]+\.[0-9]+(-[0-9a-z]+-[0-9a-z]+)?$"; then
  echo "Error: Git tag is not in the correct vX.Y.Z(-REV-HASH)? format." 1>&2
  echo "Current tag is $GIT_TAG." 1>&2
  exit 1
fi

# Extract the current major, minor, and patch version numbers
MAJOR=$(echo $GIT_TAG | sed -E 's/^v([0-9]+)\.[0-9]+\.[0-9]+(-[0-9a-z]+-[0-9a-z]+)?$/\1/')
MINOR=$(echo $GIT_TAG | sed -E 's/^v[0-9]+\.([0-9]+)\.[0-9]+(-[0-9a-z]+-[0-9a-z]+)?$/\1/')
PATCH=$(echo $GIT_TAG | sed -E 's/^v[0-9]+\.[0-9]+\.([0-9]+)(-[0-9a-z]+-[0-9a-z]+)?$/\1/')

echo "Current version is $MAJOR.$MINOR.$PATCH"

# Check if the argument is "major", "minor", or "patch"
if [ $# -eq 1 ]; then
  if [ "$1" = "major" ] || [ "$1" = "minor" ] || [ "$1" = "patch" ]; then
    # Bump the appropriate version number
    if [ "$1" = "major" ]; then
      MAJOR=$((MAJOR + 1))
      MINOR=0
      PATCH=0
    elif [ "$1" = "minor" ]; then
      MINOR=$((MINOR + 1))
      PATCH=0
    elif [ "$1" = "patch" ]; then
      PATCH=$((PATCH + 1))
    fi
  else
    echo "Error: Argument must be 'major', 'minor', or 'patch'." 1>&2
    exit 1
  fi
else
  echo "Error: Must specify an argument." 1>&2
  exit 1
fi

NEW_VERSION="$MAJOR.$MINOR.$PATCH"
NEW_TAG="v$NEW_VERSION"
NEW_BRANCH="release-$NEW_VERSION"

echo "Bumping version to $NEW_VERSION"

git checkout -b "$NEW_BRANCH"

# Modify Cargo.toml to use the current version
sed -i '' "s/^version = .*/version = \"$NEW_VERSION\"/" Cargo.toml

cargo build

git commit -am "release: bump version to $NEW_VERSION"

git tag $NEW_TAG
git push --atomic origin "$NEW_BRANCH" $NEW_TAG

echo "Done!"
