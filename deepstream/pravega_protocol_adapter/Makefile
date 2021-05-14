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

PRAVEGA_PROTOCOL_ADAPTER_DYLIB:=../../target/release/lib$(PRAVEGA_PROTOCOL_ADAPTER).so
TEST_BIN:=../../target/test_pravega_protocol_adapter

PRAVEGA_PROTOCOL_ADAPTER_SRC:=src/lib.rs Cargo.toml
TEST_SRCS:=test_pravega_protocol_adapter.cpp

CXXFLAGS:=-I$(DS_INC)
# specify the linked path of dynamic library of libnvds_pravega_proto.so for both compile time(by -L) and run time(by -Wl,-rpath)
LDFLAGS:=-L../../target/release -L$(DS_LIB) -l$(PRAVEGA_PROTOCOL_ADAPTER) -Wl,-rpath,'$$ORIGIN'/release -Wl,-rpath,$(DS_LIB)

default: all

all: $(PRAVEGA_PROTOCOL_ADAPTER_DYLIB) $(TEST_BIN)

$(PRAVEGA_PROTOCOL_ADAPTER_DYLIB): $(PRAVEGA_PROTOCOL_ADAPTER_SRC)
	$(CARGO) build --release

$(TEST_BIN): $(TEST_SRCS) $(PRAVEGA_PROTOCOL_ADAPTER_DYLIB)
	$(CXX) -o $@ $< $(CXXFLAGS) $(LDFLAGS)

clean:
	rm -rf $(TEST_BIN) $(PRAVEGA_PROTOCOL_ADAPTER_DYLIB)

PRAVEGA_CONTROLLER_URI ?= tcp://127.0.0.1:9090
PRAVEGA_CFG ?=
test: all
	$(TEST_BIN) $(PRAVEGA_CONTROLLER_URI) $(PRAVEGA_CFG)
