#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build --release
ls -lh ${ROOT_DIR}/gst-plugin-pravega/target/release/*.so
popd
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/release:${GST_PLUGIN_PATH}
$*
