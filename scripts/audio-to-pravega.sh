#!/usr/bin/env bash

# Generate audio and write raw audio to Pravega.

set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/..)
pushd ${ROOT_DIR}/gst-plugin-pravega
cargo build
ls -lh ${ROOT_DIR}/gst-plugin-pravega/target/debug/*.so
export GST_PLUGIN_PATH=${ROOT_DIR}/gst-plugin-pravega/target/debug:${GST_PLUGIN_PATH}
# log level can be INFO, DEBUG, or LOG (verbose)
export GST_DEBUG=pravegasink:LOG
export RUST_BACKTRACE=1
export GST_DEBUG_DUMP_DOT_DIR=/tmp/gst-dot/audio-to-pravega
mkdir -p ${GST_DEBUG_DUMP_DOT_DIR}
STREAM=${STREAM:-audio1}

gst-launch-1.0 \
-v \
  audiotestsrc name=src is-live=true do-timestamp=true num-buffers=100 \
! "audio/x-raw,format=S16LE,layout=interleaved,rate=44100,channels=1" \
! pravegasink stream=examples/${STREAM} sync=false \
|& tee /tmp/audio-to-pravega.log

gst-launch-1.0 \
-v \
pravegasrc stream=examples/${STREAM} \
! "audio/x-raw,format=S16LE,layout=interleaved,rate=44100,channels=1" \
! audioconvert \
! autoaudiosink \
|& tee /tmp/pravega-to-audio.log
