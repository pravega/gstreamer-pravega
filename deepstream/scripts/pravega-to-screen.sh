#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build --release
ls -lh ${ROOT_DIR}/gst-plugin-pravega/target/release/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/release:${GST_PLUGIN_PATH}
export GST_DEBUG="pravegasrc:LOG,mpegtsbase:4,mpegtspacketizer:4,GST_TRACER:7"
export RUST_BACKTRACE=1
STREAM=${STREAM:-camera8}
PRAVEGA_CONTROLLER=192.168.1.123:9090
export GST_DEBUG_DUMP_DOT_DIR=/tmp/gst-dot/pravega-to-screen
mkdir -p ${GST_DEBUG_DUMP_DOT_DIR}

gst-launch-1.0 \
-v \
pravegasrc stream=examples/${STREAM} controller=${PRAVEGA_CONTROLLER} \
! decodebin \
! videoconvert \
! autovideosink sync=false
