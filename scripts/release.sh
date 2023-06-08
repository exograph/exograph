#!/bin/sh

# Get the current banch and verify that it is main
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
if [ "$CURRENT_BRANCH" != "main" ]; then
  echo "Error: You must release it from the main branch (Current branch is $CURRENT_BRANCH)." 1>&2
  exit 1
fi

# Get the current git tag, which must be in the vX.Y.Z format
GIT_TAG=$(git describe --tags --always)

# Verify that the tag is in the correct format
if ! echo $GIT_TAG | grep -q "^v[0-9]\+\.[0-9]\+\.[0-9]\+$"; then
  echo "Error: Git tag is not in the correct vX.Y.Z format." 1>&2
  echo "Current tag is $GIT_TAG." 1>&2
  exit 1
fi

# Extract the current major, minor, and patch version numbers
MAJOR=$(echo $GIT_TAG | sed 's/v\([0-9]\+\)\.[0-9]\+\.[0-9]\+/\1/')
MINOR=$(echo $GIT_TAG | sed 's/v[0-9]\+\.\([0-9]\+\)\.[0-9]\+/\1/')
PATCH=$(echo $GIT_TAG | sed 's/v[0-9]\+\.[0-9]\+\.\([0-9]\+\)/\1/')

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

echo "Bumping version to $NEW_VERSION"

# Modify Cargo.toml to use the current version
sed -i "s/^version = .*/version = \"$NEW_VERSION\"/" Cargo.toml

git commit -am "Bump version to $NEW_VERSION"

git tag $NEW_TAG
git push origin main
git push origin $NEW_TAG

