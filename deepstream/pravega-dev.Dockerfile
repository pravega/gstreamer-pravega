ARG FROM_IMAGE

FROM ${FROM_IMAGE}

# Install Python Bindings for DeepStream.

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
        less \
        python3-dev \
        python3-gi \
        python3-gst-1.0 \
        wget

RUN cd /opt/nvidia/deepstream/deepstream/lib && \
    python3 setup.py install && \
    cd /opt/nvidia/deepstream/deepstream/sources && \
    git clone https://github.com/NVIDIA-AI-IOT/deepstream_python_apps

# Install Rust compiler.
# Based on:
#   - https://github.com/rust-lang/docker-rust-nightly/blob/master/buster/Dockerfile
#   - https://hub.docker.com/layers/rust/library/rust/1.49.0/images/sha256-71e239392f5a70bc034522a089175bd36d1344205625047ed42722a205b683b2?context=explore

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    RUST_VERSION=1.51.0

RUN set -eux; \
    rustArch="x86_64-unknown-linux-gnu"; \
    url="https://static.rust-lang.org/rustup/archive/1.23.1/${rustArch}/rustup-init"; \
    wget --quiet "$url"; \
    chmod +x rustup-init; \
    ./rustup-init -y --no-modify-path --default-toolchain $RUST_VERSION --default-host ${rustArch}; \
    rm rustup-init; \
    chmod -R a+w $RUSTUP_HOME $CARGO_HOME; \
    rustup --version; \
    cargo --version; \
    rustc --version;

# Build GStreamer Pravega libraries and applications.

WORKDIR /usr/src/gstreamer-pravega

## Build gst-plugin-pravega.

COPY gst-plugin-pravega gst-plugin-pravega
COPY pravega-client-rust pravega-client-rust
COPY pravega-video pravega-video

RUN cd gst-plugin-pravega && \
    cargo build --release && \
    mv -v target/release/*.so /usr/lib/x86_64-linux-gnu/gstreamer-1.0/

## Build pravega-video-server.

COPY pravega-video-server pravega-video-server

RUN cd pravega-video-server && \
    cargo install --path .

## Build pravega_protocol_adapter.

COPY deepstream/pravega_protocol_adapter deepstream/pravega_protocol_adapter

RUN cd deepstream/pravega_protocol_adapter && \
    cargo build --release && \
    mv -v target/release/*.so /opt/nvidia/deepstream/deepstream/lib/
