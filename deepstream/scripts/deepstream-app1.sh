#!/usr/bin/env bash
set -ex
ROOT_DIR=$(readlink -f $(dirname $0)/../..)
pushd ${ROOT_DIR}
~/deepstream/deepstream/bin/deepstream-app -c ${ROOT_DIR}/deepstream/configs/config1.txt
