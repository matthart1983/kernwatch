#!/usr/bin/env bash
# The release workflow's first job: refuse a tag that is not a finished release.
#
#   scripts/release-guard.sh vX.Y.Z
#
# Run from a full checkout of the tag. Fails unless the tag is "v" plus
# Cargo.toml's version, docs/RELEASE_NOTES.md opens with that version, and the
# commit is on origin/main. CI is not awaited: the build job repeats its checks
# on all four targets before anything publishes.
set -euo pipefail

tag="${1:?usage: release-guard.sh vX.Y.Z}"

# `::error::` marks the line as the failure in the Actions log.
die() { echo "::error::$*"; exit 1; }

version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
[ "v${version}" = "${tag}" ] || die "${tag} is not the crate's version: Cargo.toml says ${version}"

heading=$(sed -n '/^## /{p;q;}' docs/RELEASE_NOTES.md)
[ "${heading}" = "## kernwatch ${tag}" ] ||
    die "docs/RELEASE_NOTES.md opens with '${heading}', not '## kernwatch ${tag}'"

git merge-base --is-ancestor HEAD origin/main ||
    die "$(git rev-parse HEAD) is not on origin/main. Merge it first, then tag the merge."

echo "${tag} is the crate's version, has release notes and is on main"
