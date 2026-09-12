#!/bin/sh
set -eu
cd "$(dirname "$0")"
# Keep paths deterministic across developer, CI, and release workspaces.
compiler=${CLANG:-clang}
for arch in x86_64 aarch64; do
    object=kernwatch.bpf.o
    define=
    if [ "$arch" = aarch64 ]; then
        object=kernwatch-aarch64.bpf.o
        define=-DKERNWATCH_ARM64
    fi
    "$compiler" -target bpf -O2 -g -fdebug-prefix-map="$PWD"=. -ffile-prefix-map="$PWD"=. -Wall -Werror $define -c kernwatch.bpf.c -o "$object"
done
