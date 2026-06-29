#!/bin/bash
set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check arguments
if [ $# -ne 1 ]; then
    echo -e "${RED}Error: Exactly one argument required${NC}"
    echo "Usage: $0 <major|minor|patch>"
    exit 1
fi

BUMP_TYPE=$1

if [[ ! "$BUMP_TYPE" =~ ^(major|minor|patch)$ ]]; then
    echo -e "${RED}Error: Invalid bump type '$BUMP_TYPE'${NC}"
    echo "Usage: $0 <major|minor|patch>"
    exit 1
fi

# Check git status is clean
if [ -n "$(git status --porcelain)" ]; then
    echo -e "${RED}Error: Git working directory is not clean${NC}"
    echo "Please commit or stash your changes before bumping version"
    git status --short
    exit 1
fi

# Get current version from dataverse-tui Cargo.toml
CURRENT_VERSION=$(grep '^version = ' dataverse-tui/Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')

if [ -z "$CURRENT_VERSION" ]; then
    echo -e "${RED}Error: Could not find version in dataverse-tui/Cargo.toml${NC}"
    exit 1
fi

echo -e "${YELLOW}Current version: ${CURRENT_VERSION}${NC}"

# Parse version components
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"

# Bump version
case $BUMP_TYPE in
    major)
        MAJOR=$((MAJOR + 1))
        MINOR=0
        PATCH=0
        ;;
    minor)
        MINOR=$((MINOR + 1))
        PATCH=0
        ;;
    patch)
        PATCH=$((PATCH + 1))
        ;;
esac

NEW_VERSION="${MAJOR}.${MINOR}.${PATCH}"
echo -e "${GREEN}New version: ${NEW_VERSION}${NC}"

# Update dataverse-tui Cargo.toml
sed -i "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" dataverse-tui/Cargo.toml

echo -e "${YELLOW}Updated dataverse-tui/Cargo.toml${NC}"

# Verify workspace still checks
cargo check -p dataverse-tui --quiet

echo -e "${YELLOW}Checked dataverse-tui${NC}"

# Commit changes
git add dataverse-tui/Cargo.toml Cargo.lock
git commit -m "chore: bump dataverse-tui to v${NEW_VERSION}"

echo -e "${GREEN}Committed version bump${NC}"

# Create and push tag
git tag "v${NEW_VERSION}"
echo -e "${GREEN}Created tag v${NEW_VERSION}${NC}"

git push origin main
git push origin "v${NEW_VERSION}"

echo -e "${GREEN}✓ Successfully bumped dataverse-tui to v${NEW_VERSION} and pushed to remote${NC}"
echo -e "${YELLOW}GitHub Actions will now build and create a release${NC}"
