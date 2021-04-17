#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/../..)
BUILDDIR=${ROOT_DIR}/gst-build/gst-build-1.18/builddir
ninja -C ${BUILDDIR} devenv
