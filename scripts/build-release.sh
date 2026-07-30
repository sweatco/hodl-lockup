#!/bin/bash
set -eox pipefail

HOST_DIR="${HOST_DIR:-$(pwd)}"

docker run \
     --rm \
     --mount type=bind,source=$HOST_DIR,target=/host \
     --cap-add=SYS_PTRACE \
     --security-opt seccomp=unconfined \
     --platform linux/amd64 \
     -t sourcescan/cargo-near:0.13.4-rust-1.85.0@sha256:a9d8bee7b134856cc8baa142494a177f2ba9ecfededfcdd38f634e14cca8aae2 \
     /bin/bash -c "cd /host && make build"
