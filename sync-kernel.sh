#!/usr/bin/env bash

set -e

krepo="$1"
urepo=$(git rev-parse --show-toplevel)
patches=$(mktemp -d)
commit="$2"

if [ ! -d "$krepo" ]; then
    echo "expected the kernel directory as \$1, but got \"$krepo\" which either doesn't exist or is not a directory"
    exit 1
fi

pushd "$urepo" > /dev/null

if ! git cat-file -e "$commit^{commit}"; then
    echo "commit $commit does not seem to exist."
    exit 1
fi

popd > /dev/null # $urepo


echo "Copying all commits in \"$urepo\" into \"$krepo\" from commit $commit onwards:"
git log --oneline "$commit..HEAD"
read -p "Does this look good to you? [Y/n] " ans
case "$ans" in
    Y)
        ;;
    y)
        ;;
    *)
        exit 1
        ;;
esac

krepo=$(realpath "$krepo")
urepo=$(realpath "$urepo")
patches=$(realpath "$patches")

pushd "$urepo" > /dev/null

git format-patch --quiet --output-directory "$patches" "$commit"

pushd "$patches" > /dev/null

sed -i 's/^Subject: \[PATCH .*\] /&rust: pin-init: /' *

popd > /dev/null # $patches

popd > /dev/null # $urepo



pushd "$krepo" > /dev/null

head=$(git rev-parse HEAD)

git am                                              \
    --signoff                                       \
    --directory="rust/pin-init"                     \
    --exclude="rust/pin-init/sync-kernel.sh"        \
    --exclude="rust/pin-init/LICENSE-APACHE"        \
    --exclude="rust/pin-init/LICENSE-MIT"           \
    --exclude="rust/pin-init/CHANGELOG.md"          \
    --exclude="rust/pin-init/flake.nix"             \
    --exclude="rust/pin-init/flake.lock"            \
    --exclude="rust/pin-init/Cargo.toml"            \
    --exclude="rust/pin-init/Cargo.lock"            \
    --exclude="rust/pin-init/build.rs"              \
    --exclude="rust/pin-init/.clippy.toml"          \
    --exclude="rust/pin-init/.github/*"             \
    --exclude="rust/pin-init/tests/*"               \
    --exclude="rust/pin-init/internal/Cargo.toml"   \
    --exclude="rust/pin-init/internal/Cargo.lock"   \
    --exclude="rust/pin-init/internal/build.rs"     \
    --empty=drop                                    \
    $patches/*

# need the `--exec 'true'` in order for the `--no-keep-empty` option to actually do stuff
git rebase --no-keep-empty --quiet --exec 'true' "$head"

popd > /dev/null # $krepo

rm $patches/*
