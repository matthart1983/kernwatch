#!/bin/sh
set -eu
cd "$(dirname "$0")"
"${CLANG:-clang}" -target bpf -O2 -g -Wall -Werror -c kernwatch.bpf.c -o kernwatch.bpf.o
"${CLANG:-clang}" -target bpf -DKERNWATCH_ARM64 -O2 -g -Wall -Werror -c kernwatch.bpf.c -o kernwatch-aarch64.bpf.o
