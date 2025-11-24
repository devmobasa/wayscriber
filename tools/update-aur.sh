#!/bin/bash
# Automated AUR package update script
# Updates PKGBUILD version, builds, tests, and pushes to AUR

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

SOURCE_FILE="$PROJECT_ROOT/packaging/PKGBUILD"
AUR_REPO="wayscriber"
PACKAGE_URL="https://aur.archlinux.org/packages/wayscriber"
AUR_DIR="$HOME/aur-packages/wayscriber"

echo "═══════════════════════════════════════════════════════════════"
echo "  WAYSCRIBER - AUR UPDATE AUTOMATION"
echo "═══════════════════════════════════════════════════════════════"
echo ""

# Check we're in the right directory
if [ ! -f "$PROJECT_ROOT/Cargo.toml" ]; then
    echo -e "${RED}❌ Error: Not in wayscriber project root${NC}"
    exit 1
fi

# Get version from Cargo.toml
CARGO_VERSION=$(grep '^version = ' "$PROJECT_ROOT/Cargo.toml" | head -1 | sed 's/version = "\(.*\)"/\1/')
echo -e "${GREEN}📦 Current version in Cargo.toml: $CARGO_VERSION${NC}"
echo ""

# Check if version tag exists on GitHub
cd "$PROJECT_ROOT"
if ! git tag | grep -q "^v$CARGO_VERSION\$"; then
    echo -e "${YELLOW}⚠️  Git tag v$CARGO_VERSION does not exist${NC}"
    echo ""
    read -p "Create and push tag v$CARGO_VERSION? (y/n) " -n 1 -r
    echo ""
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        git tag -a "v$CARGO_VERSION" -m "Release v$CARGO_VERSION"
        git push origin "v$CARGO_VERSION"
        echo -e "${GREEN}✅ Tag created and pushed${NC}"
    else
        echo -e "${RED}❌ Aborted - tag required for AUR${NC}"
        exit 1
    fi
fi

# Check AUR directory exists
if [ ! -d "$AUR_DIR" ]; then
    echo -e "${YELLOW}AUR working directory not found: $AUR_DIR${NC}"
    echo "Creating a fresh clone..."
    mkdir -p "$AUR_DIR"
    cd "$AUR_DIR"
    git init
    git remote add origin "ssh://aur@aur.archlinux.org/$AUR_REPO.git"
else
    cd "$AUR_DIR"
fi

git fetch origin 2>/dev/null || true
git checkout master 2>/dev/null || git checkout -b master
git pull --rebase origin master 2>/dev/null || true
cd "$PROJECT_ROOT"

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 1: Update PKGBUILD"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Copy template PKGBUILD
if [ ! -f "$SOURCE_FILE" ]; then
    echo -e "${RED}❌ Error: template $SOURCE_FILE not found${NC}"
    exit 1
fi

cp "$SOURCE_FILE" "$AUR_DIR/PKGBUILD"
echo -e "${GREEN}Copied $SOURCE_FILE to $AUR_DIR/PKGBUILD${NC}"

# Update version in PKGBUILD
cd "$AUR_DIR"
if grep -q '^pkgver=' PKGBUILD; then
    sed -i "s/^pkgver=.*/pkgver=$CARGO_VERSION/" PKGBUILD
fi
if grep -q '^pkgrel=' PKGBUILD; then
    sed -i "s/^pkgrel=.*/pkgrel=1/" PKGBUILD
fi

echo -e "${GREEN}✅ Updated PKGBUILD: pkgver=$CARGO_VERSION, pkgrel=1${NC}"
echo ""

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 2: Generate .SRCINFO"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

makepkg --printsrcinfo > .SRCINFO
echo -e "${GREEN}✅ Generated .SRCINFO${NC}"
echo ""

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 3: Test build locally"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

read -p "Test build locally? (y/n) " -n 1 -r
echo ""
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "Building package..."
    if makepkg -f; then
        echo -e "${GREEN}✅ Build successful${NC}"
        echo ""
        read -p "Install locally to test? (y/n) " -n 1 -r
        echo ""
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            makepkg -i
        fi
    else
        echo -e "${RED}❌ Build failed - fix errors before pushing to AUR${NC}"
        exit 1
    fi
else
    echo -e "${YELLOW}⚠️  Skipping local build test${NC}"
fi
echo ""

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "STEP 4: Commit and push to AUR"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

# Show git status
echo "Files to be committed:"
git status --short
echo ""

read -p "Push to AUR? (y/n) " -n 1 -r
echo ""
if [[ $REPLY =~ ^[Yy]$ ]]; then
    # Add and commit
    git add PKGBUILD .SRCINFO .gitignore 2>/dev/null || git add PKGBUILD .SRCINFO
    git commit -m "Update to v$CARGO_VERSION"

    # Push
    if git push origin master 2>/dev/null; then
        echo ""
        echo -e "${GREEN}✅ Successfully pushed to AUR!${NC}"
    else
        # If master doesn't exist, try pushing with -u
        git push -u origin master
        echo ""
        echo -e "${GREEN}✅ Successfully pushed to AUR!${NC}"
    fi

    echo ""
    echo "═══════════════════════════════════════════════════════════════"
    echo -e "${GREEN}✅ AUR PACKAGE UPDATED SUCCESSFULLY${NC}"
    echo "═══════════════════════════════════════════════════════════════"
    echo ""
    echo "Package URL: $PACKAGE_URL"
    echo "Version: $CARGO_VERSION"
    echo ""
    echo "Users can update with:"
    echo "  yay -Syu wayscriber"
    echo "  paru -Syu wayscriber"
    echo ""
else
    echo -e "${YELLOW}⚠️  Push to AUR cancelled${NC}"
    echo ""
    echo "To push manually later:"
    echo "  cd $AUR_DIR"
    echo "  git add PKGBUILD .SRCINFO"
    echo "  git commit -m 'Update to v$CARGO_VERSION'"
    echo "  git push origin master"
fi
