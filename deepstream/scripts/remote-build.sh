#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/../..)
pushd ${ROOT_DIR}
scripts/rsync.sh
pushd ${ROOT_DIR}/gst-plugin-pravega
ssh ${SSH_OPTS} ${SSH_HOST} "cd ~/gstreamer-pravega/gst-plugin-pravega && cargo build && ls -lh target/debug/*.so"
