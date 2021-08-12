---
title: Network Video Recorder
---

<!--
Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0
-->

## Network Video Recorder

### Recommended Hardware

- 4 CPU cores
- 16 GiB RAM
- 300 GB for OS and applications
- 1 TB for video storage

### Setup Procedure

- Install Ubuntu Desktop 21.04.

```bash
sudo apt-get install \
  docker \
  docker-compose \
  git \
  openjdk-11-jdk
sudo usermod -a -G docker ${USER}
```

Use Disks application to mount the video storage disk on /mnt/data1.

```bash
git clone --recursive https://github.com/pravega/gstreamer-pravega
cd gstreamer-pravega
RUST_JOBS=4 BUILD_PROD=0 docker/build-release.sh
```

### Start Pravega Server

```bash
git clone https://github.com/pravega/pravega
cd pravega
./gradlew docker
cd ~/gstreamer-pravega/pravega-docker
export HOST_IP=192.168.1.131
export PRAVEGA_LTS_PATH=/mnt/data1/pravega-lts
docker-compose up -d
```

### Start Camera Recorder

```bash
export CAMERA_URI=rtsp://localhost:554/live
export PRAVEGA_STREAM=camera001
export PRAVEGA_CONTROLLER_URI=tcp://${HOST_IP}:9090
export PRAVEGA_SCOPE=default
export DOCKER_IMAGE=pravega/gstreamer:pravega-dev
export PRAVEGA_RETENTION_POLICY_TYPE=bytes
export PRAVEGA_RETENTION_BYTES=1000000000
docker run --detach --restart always \
    --name ${PRAVEGA_STREAM}-recorder \
    -e CAMERA_URI \
    -e PRAVEGA_STREAM \
    -e PRAVEGA_CONTROLLER_URI \
    -e PRAVEGA_SCOPE \
    -e PRAVEGA_RETENTION_POLICY_TYPE \
    -e PRAVEGA_RETENTION_BYTES \
    ${DOCKER_IMAGE} \
    rtsp-camera-to-pravega.py
docker logs ${PRAVEGA_STREAM}-recorder --follow
```

### Start Video Server

```bash
export PRAVEGA_CONTROLLER_URI=tcp://${HOST_IP}:9090
export DOCKER_IMAGE=pravega/gstreamer:pravega-dev
docker run --detach --restart always \
    --name pravega-video-server \
    -e PRAVEGA_CONTROLLER_URI \
    -p 3030:3030 \
    --workdir /usr/src/gstreamer-pravega/pravega-video-server \
    ${DOCKER_IMAGE} \
    pravega-video-server
```

Open your browser to:

http://localhost:3030/player?scope=default&stream=camera001
