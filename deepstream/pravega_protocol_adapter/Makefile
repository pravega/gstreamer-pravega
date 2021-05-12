#
# Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
CXX:=g++
CARGO:=cargo
DS_INC:=/opt/nvidia/deepstream/deepstream/sources/includes
DS_LIB:=/opt/nvidia/deepstream/deepstream/lib
PRAVEGA_PROTOCOL_ADAPTER:=nvds_pravega_proto

PRAVEGA_PROTOCOL_ADAPTER_DYLIB:=target/release/lib$(PRAVEGA_PROTOCOL_ADAPTER).so
TEST_BIN:=target/test_pravega_protocol_adapter

PRAVEGA_PROTOCOL_ADAPTER_SRC:=src/lib.rs Cargo.toml
TEST_SRCS:=test_pravega_protocol_adapter.cpp

CXXFLAGS:=-I$(DS_INC)
LDFLAGS:=-L./target/release -L$(DS_LIB) -l$(PRAVEGA_PROTOCOL_ADAPTER) -Wl,-rpath ./target/release -Wl,-rpath=$(DS_LIB)

default: all

all: $(PRAVEGA_PROTOCOL_ADAPTER_DYLIB) $(TEST_BIN)

$(PRAVEGA_PROTOCOL_ADAPTER_DYLIB): $(PRAVEGA_PROTOCOL_ADAPTER_SRC)
	$(CARGO) build --release

$(TEST_BIN): $(TEST_SRCS) $(PRAVEGA_PROTOCOL_ADAPTER_DYLIB)
	$(CXX) -o $@ $< $(CXXFLAGS) $(LDFLAGS)

clean:
	rm -rf $(TEST_BIN) $(PRAVEGA_PROTOCOL_ADAPTER_DYLIB)

test: all
	pravega_client_tls_cert_path=/etc/ssl/certs/ca-certificates.crt $(TEST_BIN) \
		"tls://pravega-controller.kubespray.nautilus-platform-dev.com:443" \
		"/home/ubuntu/cfg_pravega.txt"
