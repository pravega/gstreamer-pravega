#!/usr/bin/env bash
set -ex

ROOT_DIR=$(readlink -f -- "$(dirname -- "$0")/..")

export DATA_DIR=${DATA_DIR:-${ROOT_DIR}}

docker run \
-d \
-p 8888:8888 \
-e JUPYTER_ENABLE_LAB=yes \
-v "${ROOT_DIR}":/home/jovyan/gstreamer-pravega \
-v "${DATA_DIR}":/home/jovyan/data \
--name jupyter-notebook-gstreamer-pravega \
jupyter/scipy-notebook:6d42503c684f \
jupyter-lab \
--ip=0.0.0.0 \
--no-browser

sleep 5s

docker logs jupyter-notebook-gstreamer-pravega
