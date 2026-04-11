#!/usr/bin/env bash
set -euo pipefail

REPO="Aatamte/auriga"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

# Detect platform
OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}-${ARCH}" in
    Darwin-arm64)  TARGET="aarch64-apple-darwin" ;;
    Darwin-x86_64) TARGET="x86_64-apple-darwin" ;;
    Linux-x86_64)  TARGET="x86_64-unknown-linux-gnu" ;;
    *)
        echo "Unsupported platform: ${OS}-${ARCH}" >&2
        exit 1
        ;;
esac

# Get latest release tag
if [ -n "${VERSION:-}" ]; then
    TAG="$VERSION"
else
    TAG="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name"' | cut -d'"' -f4)"
fi

if [ -z "$TAG" ]; then
    echo "Could not determine latest version." >&2
    exit 1
fi

URL="https://github.com/${REPO}/releases/download/${TAG}/auriga-${TARGET}.tar.gz"

echo "Installing auriga ${TAG} for ${TARGET}..."
echo "  from: ${URL}"
echo "  to:   ${INSTALL_DIR}"

# Download and extract
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

curl -fsSL "$URL" -o "${TMP}/auriga.tar.gz"
tar xzf "${TMP}/auriga.tar.gz" -C "$TMP"

# Install
mkdir -p "$INSTALL_DIR"
cp "${TMP}/auriga" "${INSTALL_DIR}/auriga"
cp "${TMP}/auriga-app" "${INSTALL_DIR}/auriga-app"
chmod +x "${INSTALL_DIR}/auriga" "${INSTALL_DIR}/auriga-app"

echo ""
echo "Installed auriga ${TAG} to ${INSTALL_DIR}"

# Check PATH
if ! echo "$PATH" | tr ':' '\n' | grep -q "^${INSTALL_DIR}$"; then
    echo ""
    echo "Add ${INSTALL_DIR} to your PATH:"
    echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
fi
