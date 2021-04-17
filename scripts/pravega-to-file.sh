#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build
ls -lh ${ROOT_DIR}/gst-plugin-pravega/target/debug/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/debug:${GST_PLUGIN_PATH}
export GST_DEBUG=pravegasrc:LOG
export RUST_BACKTRACE=1
STREAM=${STREAM:-camera8}

gst-launch-1.0 \
-v \
pravegasrc stream=examples/${STREAM} \
! filesink location=/mnt/data/tmp/test3.ts

ls -l /mnt/data/tmp/test3.ts
