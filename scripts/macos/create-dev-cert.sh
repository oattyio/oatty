#!/usr/bin/env bash
set -euo pipefail

# Create a self-signed macOS code signing certificate for local development.
# This certificate will be placed in the login keychain and can be used with `codesign`.
#
# Usage:
#   scripts/macos/create-dev-cert.sh [Common-Name]
#
# Example:
#   scripts/macos/create-dev-cert.sh "next-gen-cli-dev (LOCAL)"
#   export NEXTGEN_CODESIGN_ID="next-gen-cli-dev (LOCAL)"
#   scripts/macos/sign.sh target/debug/next-gen-cli
#
# The script configures Keychain access so Apple developer tools (codesign, debugserver)
# can use the identity without repeated prompts. If configuration fails, follow the
# printed instructions to update the partition list manually.

CN=${1:-"next-gen-cli-dev (LOCAL)"}
KEYCHAIN_NAME=${KEYCHAIN:-login.keychain-db}

# Resolve keychain path: allow passing either a name or a full path, and fallback to legacy login.keychain
if [[ "${KEYCHAIN_NAME}" == */* ]]; then
  KEYCHAIN_PATH="${KEYCHAIN_NAME}"
else
  KEYCHAIN_PATH="${HOME}/Library/Keychains/${KEYCHAIN_NAME}"
fi
if [[ ! -f "${KEYCHAIN_PATH}" && -f "${HOME}/Library/Keychains/login.keychain" ]]; then
  KEYCHAIN_PATH="${HOME}/Library/Keychains/login.keychain"
fi

cat <<EOF
This script will create a self-signed code signing certificate with Common Name:
  ${CN}
In keychain:
  ${KEYCHAIN_PATH}
EOF

read -r -p "Proceed? [y/N] " yn
case "$yn" in
  [Yy]*) ;;
  *) echo "Aborted."; exit 1;;
 esac

TMPDIR=$(mktemp -d)
CERT_PATH="${TMPDIR}/dev.cer"
KEY_PATH="${TMPDIR}/dev.key"
P12_PATH="${TMPDIR}/dev.p12"

cleanup() {
  rm -rf "${TMPDIR}"
}
trap cleanup EXIT

NEEDS_PARTITION_HINT=0

# Configure the imported identity so codesign, debug tools, and other Apple utilities can
# access it without repeated prompts. Returns 0 on success, 1 when manual intervention is
# required (for example, when the keychain password must be provided interactively).
configure_key_access() {
  local keychain_path="$1"
  local certificate_common_name="$2"
  local keychain_password="$3"

  local security_arguments=(
    -S
    "apple-tool:,apple:"
    -s
  )

  if [[ -n "${certificate_common_name}" ]]; then
    security_arguments+=( -D "${certificate_common_name}" )
  fi

  if [[ -n "${keychain_password}" ]]; then
    security_arguments+=( -k "${keychain_password}" )
    /usr/bin/security set-key-partition-list "${security_arguments[@]}" "${keychain_path}"
    return $?
  fi

  security_arguments+=( -k "" )
  /usr/bin/security set-key-partition-list "${security_arguments[@]}" "${keychain_path}" 2>/dev/null
}

# Generate a non-empty temporary password for PKCS#12 export (empty passwords can fail to import on macOS)
P12_PASS=$(LC_ALL=C tr -dc 'A-Za-z0-9' </dev/urandom | head -c 24 || true)
if [[ -z "${P12_PASS}" ]]; then P12_PASS="dev-cert-pass-$(date +%s)"; fi

# Create an OpenSSL config to add proper Code Signing extensions
CONF_PATH="${TMPDIR}/openssl.cnf"
cat > "${CONF_PATH}" <<CONF
[req]
distinguished_name = dn
x509_extensions = codesign
prompt = no

[dn]
CN = ${CN}

[codesign]
basicConstraints = CA:false
keyUsage = critical,digitalSignature
extendedKeyUsage = codeSigning
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid,issuer
CONF

# Create a private key and self-signed certificate suitable for codesign (with EKU=codeSigning)
/usr/bin/openssl req -new -newkey rsa:2048 -x509 -days 3650 -nodes \
  -config "${CONF_PATH}" -extensions codesign \
  -keyout "${KEY_PATH}" -out "${CERT_PATH}"

# Package into a PKCS#12 so we can import into keychain
/usr/bin/openssl pkcs12 -export -out "${P12_PATH}" -inkey "${KEY_PATH}" -in "${CERT_PATH}" -passout pass:"${P12_PASS}"

# Import into the target keychain
/usr/bin/security import "${P12_PATH}" -k "${KEYCHAIN_PATH}" -P "${P12_PASS}" -f pkcs12 -T /usr/bin/codesign

# Allow codesign and other Apple tools to access the imported identity without interactive prompts.
# If the keychain is password-protected, supply KEYCHAIN_PASSWORD env var or run the printed command manually.
if ! configure_key_access "${KEYCHAIN_PATH}" "${CN}" "${KEYCHAIN_PASSWORD:-}"; then
  echo "Warning: failed to update key partition list with provided KEYCHAIN_PASSWORD." >&2
  NEEDS_PARTITION_HINT=1
fi

# Find the identity name as recognized by codesign
IDENTITY_DISPLAY=$(/usr/bin/security find-identity -v -p codesigning "${KEYCHAIN_PATH}" | grep -F "${CN}" | head -n1 | sed -E 's/^\s*[0-9]+\) [A-F0-9]{40} "(.+)".*/\1/')

cat <<EOF

Created self-signed code signing identity:
  ${IDENTITY_DISPLAY:-${CN}}

Use it like:
  NEXTGEN_CODESIGN_ID="${IDENTITY_DISPLAY:-${CN}}" NEXTGEN_CODESIGN_BIN=target/debug/next-gen-cli scripts/macos/sign.sh

You may also set Keychain item Access Control to allow applications signed by this certificate
for seamless keychain access.
EOF

if [[ "${NEEDS_PARTITION_HINT}" == "1" ]]; then
  cat <<EOF

Note: Could not automatically update key partition settings. If signing prompts persist, run:
  security set-key-partition-list -S apple-tool:,apple: -s -D "${CN}" -k "<KEYCHAIN_PASSWORD>" "${KEYCHAIN_PATH}"

Replace <KEYCHAIN_PASSWORD> with the password for the target keychain, or rerun this script with
KEYCHAIN_PASSWORD set in the environment to perform the update automatically.
EOF
fi
