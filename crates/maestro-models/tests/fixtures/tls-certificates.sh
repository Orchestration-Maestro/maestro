#!/bin/sh
# Re-issues the TLS server certificates the provider tests trust, from the committed keys.
#
# Every certificate starts on 2019-06-01. Apple's TLS policy refuses a server certificate
# issued on or after 2019-07-01 that is valid for more than 825 days, and these fixtures
# must not expire, so they predate that rule and stay valid until 2126.
#
# Run from any directory: sh crates/maestro-models/tests/fixtures/tls-certificates.sh
set -eu
fixtures=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# Signs `$work/$1.csr` with the extensions of section `$1`, using `$2` and its certificate
# `$3` (none for a self-signed certificate), and writes `$4`.
sign() {
    : > "$work/index.txt"
    cat > "$work/ca.cnf" <<EOF
[ca]
default_ca = fixtures
[fixtures]
database = $work/index.txt
new_certs_dir = $work
rand_serial = yes
default_md = sha256
policy = anything
copy_extensions = none
unique_subject = no
[anything]
commonName = supplied
[authority]
basicConstraints = critical, CA:TRUE
keyUsage = critical, keyCertSign, cRLSign
subjectKeyIdentifier = hash
[localhost]
basicConstraints = critical, CA:FALSE
keyUsage = critical, digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
subjectAltName = DNS:localhost, IP:127.0.0.1
authorityKeyIdentifier = keyid
[destination]
basicConstraints = critical, CA:FALSE
keyUsage = critical, digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
subjectAltName = DNS:destination.invalid, DNS:localhost
EOF
    section=$1 key=$2 output=$4
    if [ -z "$3" ]; then set -- -selfsign; else set -- -cert "$3"; fi
    openssl ca -batch -notext -config "$work/ca.cnf" -extensions "$section" \
        -startdate 20190601000000Z -enddate 21261018000000Z \
        -keyfile "$key" "$@" -in "$work/$section.csr" -out "$output"
}

# A key in the committed DER form, as the PEM the signing commands read.
pem() {
    openssl pkey -inform DER -in "$1" -out "$2"
}

# The cloud-authentication server, signed by its own authority. The client authority and
# client certificate stay as they are, so the recorded chain hashes keep their values.
openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -out "$work/authority.key"
openssl req -new -key "$work/authority.key" -subj "/CN=Controlled Maestro ADC server authority" \
    -out "$work/authority.csr"
sign authority "$work/authority.key" "" "$fixtures/cloud-authentication/server-ca.pem"
pem "$fixtures/cloud-authentication/server-key.der" "$work/server.key"
openssl req -new -key "$work/server.key" -subj "/CN=localhost" -out "$work/localhost.csr"
sign localhost "$work/authority.key" "$fixtures/cloud-authentication/server-ca.pem" \
    "$fixtures/cloud-authentication/server.pem"

# The proxy and destination server, which is its own trust anchor.
pem "$fixtures/socks-tls/key.der" "$work/destination.key"
openssl req -new -key "$work/destination.key" -subj "/CN=destination.invalid" \
    -out "$work/destination.csr"
sign destination "$work/destination.key" "" "$fixtures/socks-tls/cert.pem"
openssl x509 -in "$fixtures/socks-tls/cert.pem" -outform DER -out "$fixtures/socks-tls/cert.der"
