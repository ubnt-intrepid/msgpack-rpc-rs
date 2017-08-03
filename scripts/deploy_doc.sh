#!/bin/sh

set -euo pipefail

TRAVIS_BRANCH=$1
TRAVIS_RUST_VERSION=$2
GH_TOKEN=$3
TRAVIS_REPO_SLUG=$4

if [[ "${TRAVIS_BRANCH:-}" = "master" ]] && [[ "${TRAVIS_RUST_VERSION}" = "stable" ]]; then
    cargo doc -p msgpack-rpc --all-features --no-deps

    cargo install --force cobalt-bin
    cobalt build -s site -d target/doc

    ghp-import -n target/doc
    git push -qf "https://${GH_TOKEN}@github.com/${TRAVIS_REPO_SLUG}.git" gh-pages
fi
