#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/../..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo deb
DEB_FILE=${ROOT_DIR}/gst-plugin-pravega/target/debian/gst-plugin-pravega_0.7.0_arm64.deb
ls -lh ${DEB_FILE}
sudo dpkg -i ${DEB_FILE}
ls -lh /usr/lib/aarch64-linux-gnu/gstreamer-1.0/libgstpravega.so
gst-inspect-1.0 pravega
