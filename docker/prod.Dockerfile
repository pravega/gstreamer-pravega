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

FROM pravega/gstreamer:dev-downloaded as dev-downloaded

FROM "${DOCKER_REPOSITORY}ubuntu:20.10" as prod-base

RUN \
    apt-get update && \
    apt-get dist-upgrade -y && \
    apt-get install -y --no-install-recommends \
        bubblewrap \
        ca-certificates \
        intel-media-va-driver-non-free \
        iso-codes \
        ladspa-sdk \
        liba52-0.7.4 \
        libaa1 \
        libaom0 \
        libass9 \
        libavcodec58 \
        libavfilter7 \
        libavformat58 \
        libavutil56 \
        libbs2b0 \
        libbz2-1.0 \
        libcaca0 \
        libcap2 \
        libchromaprint1 \
        libcurl3-gnutls \
        libdca0 \
        libde265-0 \
        libdv4 \
        libdvdnav4 \
        libdvdread8 \
        libdw1 \
        libegl1 \
        libepoxy0 \
        libfaac0 \
        libfaad2 \
        libfdk-aac2 \
        libflite1 \
        libfluidsynth2 \
        libgbm1 \
        libgcrypt20 \
        libgl1 \
        libgles1 \
        libgles2 \
        libglib2.0-0 \
        libgme0 \
        libgmp10 \
        libgsl25 \
        libgsm1 \
        libgudev-1.0-0 \
        libharfbuzz-icu0 \
        libjpeg8 \
        libkate1 \
        liblcms2-2 \
        liblilv-0-0 \
        libmfx1 \
        libmjpegutils-2.1-0 \
        libmodplug1 \
        libmp3lame0 \
        libmpcdec6 \
        libmpeg2-4 \
        libmpg123-0 \
        libofa0 \
        libogg0 \
        libopencore-amrnb0 \
        libopencore-amrwb0 \
        libopenexr25 \
        libopenjp2-7 \
        libopus0 \
        liborc-0.4-0 \
        libpango-1.0-0 \
        libpng16-16 \
        librsvg2-2 \
        librtmp1 \
        libsbc1 \
        libseccomp2 \
        libshout3 \
        libsndfile1 \
        libsoundtouch1 \
        libsoup2.4-1 \
        libspandsp2 \
        libspeex1 \
        libsrt1 \
        libsrtp2-1 \
        libssl1.1 \
        libtag1v5 \
        libtheora0 \
        libtwolame0 \
        libunwind8 \
        libva2 \
        libvisual-0.4-0 \
        libvo-aacenc0 \
        libvo-amrwbenc0 \
        libvorbis0a \
        libvpx6 \
        libvulkan1 \
        libwavpack1 \
        libwayland-client0 \
        libwayland-egl1 \
        libwayland-server0 \
        libwebp6 \
        libwebpdemux2 \
        libwebpmux3 \
        libwebrtc-audio-processing1 \
        libwildmidi2 \
        libwoff1 \
        libx264-160 \
        libx265-192 \
        libxkbcommon0 \
        libxslt1.1 \
        libzbar0 \
        libzvbi0 \
        mjpegtools \
        wayland-protocols \
        xdg-dbus-proxy \
    && \
    apt-get clean && \
    rm -rf /var/lib/apt/lists/*


FROM dev-downloaded as debug-prod-compile
ENV DEBUG=true
ENV OPTIMIZATIONS=true
RUN ["/compile"]

FROM pravega/gstreamer:prod-base as debug-prod
COPY --from=debug-prod-compile /compiled-binaries /

FROM dev-downloaded as prod-compile
ENV DEBUG=false
ENV OPTIMIZATIONS=true
RUN ["/compile"]

FROM pravega/gstreamer:prod-base as prod
COPY --from=prod-compile /compiled-binaries /
