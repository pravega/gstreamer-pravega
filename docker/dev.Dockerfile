#
# Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#

ARG DOCKER_REPOSITORY=""

FROM "${DOCKER_REPOSITORY}ubuntu:20.10" as base

COPY docker/build-gstreamer/install-dependencies /

RUN ["/install-dependencies"]

COPY ca-certificates /usr/local/share/ca-certificates/
RUN update-ca-certificates

FROM base as download
# Below includes default repostories and checkout branch/commit.
# Most will be overridden by build-release.sh.

ARG GSTREAMER_REPOSITORY=https://gitlab.freedesktop.org/nazar-pc/gstreamer.git
ARG GSTREAMER_CHECKOUT=master

ARG GST_PLUGINS_BASE_REPOSITORY=https://gitlab.freedesktop.org/nazar-pc/gst-plugins-base.git
ARG GST_PLUGINS_BASE_CHECKOUT=master

ARG GST_PLUGINS_BAD_REPOSITORY=https://gitlab.freedesktop.org/nazar-pc/gst-plugins-bad.git
ARG GST_PLUGINS_BAD_CHECKOUT=master

ARG GST_PLUGINS_GOOD_REPOSITORY=https://gitlab.freedesktop.org/nazar-pc/gst-plugins-good.git
ARG GST_PLUGINS_GOOD_CHECKOUT=master

ARG GST_PLUGINS_UGLY_REPOSITORY=https://gitlab.freedesktop.org/gstreamer/gst-plugins-ugly.git
ARG GST_PLUGINS_UGLY_CHECKOUT=master

ARG GST_LIBAV_REPOSITORY=https://gitlab.freedesktop.org/gstreamer/gst-libav.git
ARG GST_LIBAV_CHECKOUT=master

ARG GST_RTSP_SERVER_REPOSITORY=https://gitlab.freedesktop.org/gstreamer/gst-rtsp-server.git
ARG GST_RTSP_SERVER_CHECKOUT=master

ARG LIBNICE_REPOSITORY=https://gitlab.freedesktop.org/libnice/libnice.git
ARG LIBNICE_CHECKOUT=2b38ba23b726694293de53c90b59b28ca11746ab

ADD docker/build-gstreamer/download /

RUN ["/download"]

ADD docker/build-gstreamer/compile /


FROM download as dev-with-source
ENV DEBUG=true
ENV OPTIMIZATIONS=false
# Compile binaries with debug symbols and keep source code
RUN ["/compile"]

FROM base as FINAL
# And binaries built with debug symbols
COPY --from=dev-with-source /compiled-binaries /
