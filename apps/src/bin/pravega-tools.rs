//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

// A CLI that provides tools to manage Pravega streams.

use clap::Clap;
use std::time::{Duration, SystemTime};

use pravega_client::client_factory::ClientFactory;
use pravega_client_config::ClientConfigBuilder;
use pravega_client_shared::{Scope, Stream, ScopedStream};
use pravega_video::index::{IndexSearcher, SearchMethod, get_index_stream_name};
use pravega_video::timestamp::PravegaTimestamp;
use pravega_video::utils::{parse_controller_uri, SyncByteReader};

/// Tools to manage Pravega streams.
#[derive(Clap)]
struct Opts {
    /// Pravega controller in format "127.0.0.1:9090"
    #[clap(short, long, default_value = "127.0.0.1:9090")]
    controller: String,
    #[clap(subcommand)]
    subcmd: SubCommand,
}

#[derive(Clap)]
enum SubCommand {
    TruncateStream(TruncateStream),
}

/// Truncate a stream written by the pravegasink GStreamer plugin.
#[derive(Clap)]
struct TruncateStream {
    /// Pravega scope
    #[clap(long)]
    scope: String,
    /// Pravega stream
    #[clap(long)]
    stream: String,
    /// All data older than this many days will be deleted. Decimals are allowed.
    #[clap(long)]
    age_days: f64,
}

fn main() {
    env_logger::init();
    let opts: Opts = Opts::parse();
    match opts.subcmd {
        SubCommand::TruncateStream(c) => {
            truncate_stream(opts.controller, c.scope, c.stream, c.age_days);
        }
    }
}

fn truncate_stream(controller: String, scope_name: String, stream_name: String, age_days: f64) {
    let age_seconds = age_days * 24.0 * 60.0 * 60.0;
    let age = Duration::from_secs_f64(age_seconds);
    let truncate_at_time = SystemTime::now() - age;
    let truncate_at_timestamp: PravegaTimestamp = truncate_at_time.into();
    println!("Truncating stream {}/{} at {}", scope_name, stream_name, truncate_at_timestamp);
    let index_stream_name = get_index_stream_name(&stream_name);
    let scope = Scope::from(scope_name);
    let stream = Stream::from(stream_name);
    let index_stream = Stream::from(index_stream_name);
    let controller_uri = parse_controller_uri(controller).unwrap();
    let client_config = ClientConfigBuilder::default()
        .controller_uri(controller_uri)
        .build()
        .expect("creating config");
    let client_factory = ClientFactory::new(client_config);
    let runtime = client_factory.runtime();
    let scoped_stream = ScopedStream {
        scope: scope.clone(),
        stream: stream.clone(),
    };
    let writer = runtime.block_on(client_factory.create_byte_writer(scoped_stream));
    let index_scoped_stream = ScopedStream {
        scope: scope.clone(),
        stream: index_stream.clone(),
    };
    let index_writer = runtime.block_on(client_factory.create_byte_writer(index_scoped_stream.clone()));
    let index_reader = runtime.block_on(client_factory.create_byte_reader(index_scoped_stream.clone()));
    let mut index_searcher = IndexSearcher::new(SyncByteReader::new(index_reader, client_factory.runtime_handle()));
    let index_record = index_searcher.search_timestamp_and_return_index_offset(
        truncate_at_timestamp, SearchMethod::Before).unwrap();
    println!("Truncating prior to {:?}", index_record);
    runtime.block_on(index_writer.truncate_data_before(index_record.1 as i64)).unwrap();
    println!("Index truncated at offset {}", index_record.1);
    runtime.block_on(writer.truncate_data_before(index_record.0.offset as i64)).unwrap();
    println!("Data truncated at offset {}", index_record.0.offset);
}
