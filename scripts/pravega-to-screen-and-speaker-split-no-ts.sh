#!/usr/bin/env bash

# Audio and video will be read from separate Pravega streams without MPEG Transport Streams.
# This will read data generated by avtestsrc-to-pravega-split-no-ts.sh.

set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build
ls -lh ${ROOT_DIR}/gst-plugin-pravega/target/debug/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/debug:${GST_PLUGIN_PATH}
export GST_DEBUG="pravegasrc:INFO,basesrc:INFO,mpegtsbase:INFO,mpegtspacketizer:INFO"
export RUST_BACKTRACE=1
export GST_DEBUG_DUMP_DOT_DIR=/tmp/gst-dot/pravega-to-screen-and-speaker-split-no-ts
mkdir -p ${GST_DEBUG_DUMP_DOT_DIR}
PRAVEGA_STREAM=${PRAVEGA_STREAM:-split10}

gst-launch-1.0 \
-v \
pravegasrc stream=examples/${PRAVEGA_STREAM}-v \
! h264parse \
! avdec_h264 \
! videoconvert \
! autovideosink \
pravegasrc stream=examples/${PRAVEGA_STREAM}-a1 \
! aacparse \
! avdec_aac \
! audioconvert \
! audioresample \
! autoaudiosink
