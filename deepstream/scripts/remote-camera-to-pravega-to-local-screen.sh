#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/../..)
pushd ${ROOT_DIR}
ssh ${SSH_OPTS} ${SSH_HOST} "pkill gst-launch ; true"
deepstream/scripts/remote-build.sh
export STREAM=${STREAM:-$(uuidgen)}
ssh ${SSH_OPTS} ${SSH_HOST} "cd ~/gstreamer-pravega && \
    STREAM=${STREAM} deepstream/scripts/camera-to-pravega.sh \
    >& /tmp/gstreamer-pravega.log" &
scripts/pravega-to-screen.sh ; true
ssh ${SSH_OPTS} ${SSH_HOST} "pkill gst-launch ; true"
