//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

use anyhow::anyhow;
use clap::Clap;
use log::info;

use uuid::Uuid;

use pravega_client::client_factory::ClientFactory;
use pravega_client_config::ClientConfigBuilder;
use pravega_client_shared::{Scope, Stream, StreamConfiguration, ScopedStream, Scaling, ScaleType};

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
    let runtime = client_factory.get_runtime();
    let num_events: u64 = 1;
    let scope = Scope::from(opts.scope);
    let stream_name = format!("{}-{}", opts.stream, Uuid::new_v4());
    info!("stream_name={}", stream_name);
    let stream = Stream::from(stream_name);
    let scoped_stream = ScopedStream {
        scope: scope.clone(),
        stream: stream.clone(),
    };

    let mut writer = runtime.block_on(async {
        let controller_client = client_factory.get_controller_client();

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
        };
        controller_client.create_stream(&stream_config).await.unwrap();

        let writer = client_factory.create_event_stream_writer(scoped_stream.clone());
        writer
    });

    let write_result = runtime.block_on(async {
        let payload = "hello world".to_string().into_bytes();
        info!("Calling write_event");
        let future = writer.write_event(payload);
        let receiver = future.await;
        info!("Finished awaiting future; receiver={:?}", receiver);
        let result = receiver.await;
        info!("Finished awaiting receiver; result={:?}", result);
        result
    });
    info!("write_result={:?}", write_result);
    let write_result3 = match write_result {
        Ok(r) => r.map_err(|e| anyhow!(e)),
        Err(e) => Err(anyhow!(e)),
    };
    info!("write_result3={:?}", write_result3);

    runtime.block_on(async {
        // create event stream reader
        let reader_group_name = format!("rg{}", uuid::Uuid::new_v4()).to_string();
        let rg = client_factory.create_reader_group(scope, reader_group_name, scoped_stream).await;
        let mut reader = rg.create_reader("r1".to_string()).await;

        // read from segment
        let mut slice = reader.acquire_segment().await.expect("acquire segment");
        for _ in 0..num_events {
            let read_event = slice.next();
            info!("read_event={:?}", read_event);
            assert!(read_event.is_some(), "event slice should have event to read");
            // assert_eq!(format!("event {}", i).into_bytes(), read_event.unwrap().value.as_slice());
        }
    });

    println!("Done.");
}
