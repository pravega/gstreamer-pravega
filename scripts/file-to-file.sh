#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build
ls -lh ${ROOT_DIR}/gst-plugin-pravega/target/debug/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/debug:${GST_PLUGIN_PATH}
export GST_DEBUG=4
export RUST_BACKTRACE=full

gst-launch-1.0 --version

gst-launch-1.0 \
-v \
rsfilesrc location=/home/faheyc/nautilus/gstreamer/gstreamer-pravega/test.ts \
! filesink location=/home/faheyc/nautilus/gstreamer/gstreamer-pravega/test2.ts
