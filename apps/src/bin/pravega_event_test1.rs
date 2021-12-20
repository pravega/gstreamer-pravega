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

use std::convert::TryInto;
use uuid::Uuid;

use pravega_client::client_factory::ClientFactory;
use pravega_client_config::ClientConfigBuilder;
use pravega_client_shared::{Scope, Stream, StreamConfiguration, ScopedStream, Scaling, ScaleType};

use pravega_video::utils;

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
    /// Use byte stream writer
    #[clap(long)]
    use_byte_stream_writer: bool,
}

/// Demonstrate ability to write using the byte stream writer and read using the event reader.
fn main() {
    env_logger::init();
    let opts: Opts = Opts::parse();
    let client_config = ClientConfigBuilder::default()
        .controller_uri(opts.controller)
        .build()
        .expect("creating config");
    let client_factory = ClientFactory::new(client_config);
    let runtime = client_factory.runtime();

    runtime.block_on(async {
        let opts: Opts = Opts::parse();
        let scope = Scope::from(opts.scope);
        let stream_name = format!("{}-{}", opts.stream, Uuid::new_v4());
        info!("stream_name={}", stream_name);
        let stream = Stream::from(stream_name);
        let controller_client = client_factory.controller_client();

        let scoped_stream = ScopedStream {
            scope: scope.clone(),
            stream: stream.clone(),
        };

        // Create stream.
        let stream_config = StreamConfiguration {
            scoped_stream: ScopedStream {
                scope: scope.clone(),
                stream: stream.clone(),
            },
            scaling: Scaling {
                scale_type: ScaleType::FixedNumSegments,
                min_num_segments: 1,
                ..Default::default()
            },
            retention: Default::default(),
            tags: utils::get_video_tags(),
        };
        controller_client.create_stream(&stream_config).await.unwrap();
        let num_events: u64 = 3;

        if opts.use_byte_stream_writer {
            let scoped_stream = scoped_stream.clone();
            let mut writer = client_factory.create_byte_writer(scoped_stream).await;
            runtime.spawn(async move{
                for i in 0..num_events {
                    let payload = format!("event {}", i).into_bytes();
                    let payload_length = payload.len();
                    let event_length: u32 = payload_length.try_into().unwrap();
                    let write_length = payload_length + 8;
                    let mut bytes_to_write: Vec<u8> = vec![0; write_length];
                    bytes_to_write[4..8].copy_from_slice(&event_length.to_be_bytes()[..]);
                    bytes_to_write[8..8+payload_length].copy_from_slice(&payload[..]);
                    info!("bytes_to_write={:?}", bytes_to_write);
                    writer.write(&bytes_to_write).await.unwrap();
                }
            });
        } else {
            let mut writer = client_factory.create_event_writer(scoped_stream.clone());
            let payload = "hello world".to_string().into_bytes();
            info!("Calling write_event");
            let future = writer.write_event(payload);
            let receiver = future.await;
            info!("Finished awaiting future; receiver={:?}", receiver);
            let result = receiver.await;
            info!("Finished awaiting receiver; result={:?}", result);
        }

        // create event stream reader
        let reader_group_name = format!("rg{}", uuid::Uuid::new_v4()).to_string();
        let rg = client_factory.create_reader_group(reader_group_name, scoped_stream).await;
        let mut reader = rg.create_reader("r1".to_string()).await;

        // read from segment
        let mut slice = reader.acquire_segment().await.expect("acquire segment").unwrap();
        for i in 0..num_events {
            let read_event = slice.next();
            info!("read_event={:?}", read_event);
            assert!(read_event.is_some(), "event slice should have event to read");
            assert_eq!(format!("event {}", i).into_bytes(), read_event.unwrap().value.as_slice());
        }
    });

    println!("Done.");
}
