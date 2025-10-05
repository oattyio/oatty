set -euo pipefail

# macOS code signing helper for Rust-built binaries.
# Usage:
#   NEXTGEN_CODESIGN_BIN=target/debug/next-gen-cli NEXTGEN_CODESIGN_ID="CN=next-gen-cli-dev (LOCAL)" scripts/macos/sign.sh
#
# Required environment variables:
#   NEXTGEN_CODESIGN_BIN   Path to the binary to sign.
#   NEXTGEN_CODESIGN_ID    Signing identity (e.g., "Developer ID Application: Your Name (TEAMID)").
# Optional environment variables:
#   NEXTGEN_ENTITLEMENTS   Path to entitlements plist (defaults to macos/entitlements.plist when present).
#   NEXTGEN_CODESIGN_TIMESTAMP (true/false) Force timestamping toggle. Defaults to true for Developer ID identities.

BIN_PATH=${NEXTGEN_CODESIGN_BIN:-}
IDENTITY=${NEXTGEN_CODESIGN_ID:-${MACOS_CODESIGN_ID:-}}

if [[ -z "${BIN_PATH}" ]]; then
  echo "Error: NEXTGEN_CODESIGN_BIN must point to the binary to sign" >&2
  exit 2
fi

if [[ ! -f "${BIN_PATH}" ]]; then
  echo "Error: binary not found at: ${BIN_PATH}" >&2
  exit 2
fi

if [[ -z "${IDENTITY}" ]]; then
  cat >&2 <<'EOF'
Environment variables NEXTGEN_CODESIGN_ID (or MACOS_CODESIGN_ID) not set with a signing identity.

Example usage:
  NEXTGEN_CODESIGN_ID="CN=next-gen-cli-dev (LOCAL)" \
  NEXTGEN_CODESIGN_BIN=target/debug/next-gen-cli \
  scripts/macos/sign.sh

Tip: For local development, run scripts/macos/create-dev-cert.sh to create a self-signed identity,
then export NEXTGEN_CODESIGN_ID before invoking this script.
EOF
  exit 2
fi

# Derive project root (script lives under scripts/macos)
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)"
ROOT_DIR="$(cd -- "${SCRIPT_DIR}/../.." &> /dev/null && pwd)"
ENTITLEMENTS=${NEXTGEN_ENTITLEMENTS:-${ROOT_DIR}/macos/entitlements.plist}

if [[ ! -f "${ENTITLEMENTS}" ]]; then
  if [[ -n "${NEXTGEN_ENTITLEMENTS:-}" ]]; then
    echo "Warning: entitlements file not found at ${ENTITLEMENTS}. Proceeding without entitlements." >&2
  fi
  ENT_ARGS=()
else
  ENT_ARGS=(--entitlements "${ENTITLEMENTS}")
fi

# Hardened runtime improves trust for some setups; safe for CLI tools.
# If using a self-signed cert, omit --timestamp to avoid Apple server requirement.
TIMESTAMP_DEFAULT=false
if [[ "${IDENTITY}" == Developer\ ID\ Application:* ]]; then
  TIMESTAMP_DEFAULT=true
fi

TIMESTAMP_ENABLED=${NEXTGEN_CODESIGN_TIMESTAMP:-}
if [[ -z "${TIMESTAMP_ENABLED}" ]]; then
  TIMESTAMP_ENABLED=${TIMESTAMP_DEFAULT}
fi

declare -a CODESIGN_ARGS=("/usr/bin/codesign" --force --options runtime)

if [[ "${TIMESTAMP_ENABLED}" == "true" || "${TIMESTAMP_ENABLED}" == "1" ]]; then
  CODESIGN_ARGS+=(--timestamp)
fi

if [[ ${#ENT_ARGS[@]} -gt 0 ]]; then
  CODESIGN_ARGS+=("${ENT_ARGS[@]}")
fi

CODESIGN_ARGS+=(--sign "${IDENTITY}" "${BIN_PATH}")

set -x
"${CODESIGN_ARGS[@]}"
set +x

# Verify
/usr/bin/codesign --verify --deep --strict --verbose=2 "${BIN_PATH}"
if [[ -x /usr/sbin/spctl ]]; then
  /usr/sbin/spctl --assess --type execute --verbose "${BIN_PATH}" || true
fi

echo "Signed: ${BIN_PATH} with identity: ${IDENTITY}"
