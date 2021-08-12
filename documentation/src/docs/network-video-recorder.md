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

The following hardware is recommended to ensure sufficient performance for building from source.

- 4 CPU cores
- 16 GiB RAM
- 300 GB for OS and applications
- 1 TB for video storage

### Installation Procedure

1. Install Ubuntu Desktop 21.04.

2. Install dependencies.

    ```bash
    sudo apt-get install \
        docker \
        docker-compose \
        git \
        openjdk-11-jdk
    sudo usermod -a -G docker ${USER}
    ```

3. Use the Ubuntu Disks application to mount the video storage disk on `/mnt/data1`.
   Consider using LVM to span the file system across multiple disks.

4. Build GStreamer Pravega Docker image from source.

    ```bash
    cd
    git clone --recursive https://github.com/pravega/gstreamer-pravega
    cd gstreamer-pravega
    RUST_JOBS=4 BUILD_PROD=0 docker/build-release.sh
    ```

### Start Pravega

1. Build the Pravega Docker image from source.

    ```bash
    cd
    git clone https://github.com/pravega/pravega
    cd pravega
    ./gradlew docker
    ```

2. Run Pravega in Docker.
   In the command below, replace HOST_IP with the IP address of your Ethernet NIC.

    ```bash
    cd ~/gstreamer-pravega/pravega-docker
    export HOST_IP=192.168.1.101
    export PRAVEGA_LTS_PATH=/mnt/data1/pravega-lts
    docker-compose up -d
    ```

### Start Camera Recorder

This will run a Docker container for recording from an RTSP camera to a Pravega stream.
These steps can be repeated for each camera.
Be sure to change CAMERA_URI and PRAVEGA_STREAM for each camera.

1. Set environment variables.

    ```bash
    export CAMERA_URI=rtsp://localhost:554/live
    export PRAVEGA_STREAM=camera001
    export PRAVEGA_CONTROLLER_URI=tcp://${HOST_IP}:9090
    export PRAVEGA_SCOPE=default
    export DOCKER_IMAGE=pravega/gstreamer:pravega-dev
    export PRAVEGA_RETENTION_POLICY_TYPE=bytes
    export PRAVEGA_RETENTION_BYTES=300000000000
    ```

2. Start a camera recorder container.

    ```bash
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
    ```

3. View the application log using the following command.

    ```bash
    docker logs ${PRAVEGA_STREAM}-recorder --follow
    ```

    When working properly, you will see messages similar to the following repeated every 1 to 2 seconds.

    ```
    0:12:42.148478332     1      0x119d9e0 DEBUG            pravegasink gst-plugin-pravega/src/pravegasink/imp.rs:951:gstpravega::pravegasink::imp:<pravegasink> render: Creating index record at key frame; last index record was created 1.0658089510000002 sec ago
    0:12:42.148583334     1      0x119d9e0 DEBUG            pravegasink gst-plugin-pravega/src/pravegasink/imp.rs:1029:gstpravega::pravegasink::imp:<pravegasink> render: Wrote index record IndexRecord { timestamp: 2021-08-12T04:32:05.145146606Z (1628742762145146606 ns, 452428:32:42.145146606), offset: 90849711, random_access: true, discontinuity: false }
    ```

### Start Pravega Video Server

The Pravega Video Server will serve a web site that can be used to play videos in a browser.

1. Set environment variables.

    ```bash
    export PRAVEGA_CONTROLLER_URI=tcp://${HOST_IP}:9090
    export DOCKER_IMAGE=pravega/gstreamer:pravega-dev

2. Start Pravega Video Server.

    ```bash
    docker run --detach --restart always \
        --name pravega-video-server \
        -e PRAVEGA_CONTROLLER_URI \
        -p 3030:3030 \
        --workdir /usr/src/gstreamer-pravega/pravega-video-server \
        ${DOCKER_IMAGE} \
        pravega-video-server
    ```

3. Open your browser to:

   http://localhost:3030/player?scope=default&stream=camera001
