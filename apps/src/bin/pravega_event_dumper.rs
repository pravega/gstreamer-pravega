//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

use clap::Clap;
use log::info;
use std::{thread, time};

use pravega_client::client_factory::ClientFactory;
use pravega_client_shared::{Scope, Stream, ScopedStream};

use pravega_video::index::IndexSearcher;
use pravega_video::utils;
use pravega_video::utils::SyncByteReader;

#[derive(Clap)]
struct Opts {
    /// Pravega controller in format "127.0.0.1:9090"
    #[clap(short, long, default_value = "127.0.0.1:9090")]
    controller: String,
    /// Pravega scope
    #[clap(long)]
    scope: String,
    /// Pravega stream
    #[clap(long)]
    stream: String,
    /// Pravega keycloak file
    #[clap(long, default_value = "", setting(clap::ArgSettings::AllowEmptyValues))]
    keycloak_file: String,
}

/// Demonstrate ability to write using the byte stream writer and read using the event reader.
fn main() {
    env_logger::init();
    let opts: Opts = Opts::parse();
    let keycloak_file = if opts.keycloak_file.is_empty() {
        None
    } else {
        Some(opts.keycloak_file)
    };
    let client_config = utils::create_client_config(opts.controller, keycloak_file).expect("creating config");
    let client_factory = ClientFactory::new(client_config);
    let scope = Scope::from(opts.scope);
    let stream_name = format!("{}-index", opts.stream);
    let stream = Stream::from(stream_name);
    let index_scoped_stream = ScopedStream {
        scope: scope,
        stream: stream,
    };
    let runtime = client_factory.runtime();
    let index_reader = runtime.block_on(client_factory.create_byte_reader(index_scoped_stream));
    let mut index_searcher = IndexSearcher::new(SyncByteReader::new(index_reader, client_factory.runtime_handle()));

    let first_record = index_searcher.get_first_record().unwrap();
    info!("The first index record: timestamp={}", first_record.timestamp);
    let last_record = index_searcher.get_last_record().unwrap();
    info!("The last index record: timestamp={}", last_record.timestamp);
    let size = last_record.offset - first_record.offset;
    let size_in_mb = size / 1024 / 1024;
    info!("Data size between the first index and last index is {} MB",  size_in_mb);
}
