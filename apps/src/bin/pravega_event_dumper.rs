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

use pravega_client::client_factory::ClientFactory;
use pravega_client_shared::{Scope, Stream, ScopedStream};
use pravega_video::event_serde::EventReader;
use pravega_video::index::{IndexRecord, IndexRecordReader};
use pravega_video::utils;
use pravega_video::utils::{CurrentHead, SyncByteReader};
use std::io::{ErrorKind, Read, Seek, SeekFrom};

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
    /// Index number
    #[clap(long, default_value = "10")]
    index_num: u32,
    #[clap(long)]
    show_event: bool,
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
    let index_stream_name = format!("{}-index", opts.stream);
    let stream = Stream::from(opts.stream);
    let index_stream = Stream::from(index_stream_name);

    let stream = ScopedStream {
        scope: scope.clone(),
        stream: stream,
    };
    let index_stream = ScopedStream {
        scope: scope,
        stream: index_stream,
    };

    let runtime = client_factory.runtime();
    let byte_reader = runtime.block_on(client_factory.create_byte_reader(index_stream));
    let mut index_reader = SyncByteReader::new(byte_reader, client_factory.runtime_handle());
    let mut index_record_reader = IndexRecordReader::new();

    let byte_reader = runtime.block_on(client_factory.create_byte_reader(stream));
    let mut stream_reader = SyncByteReader::new(byte_reader, client_factory.runtime_handle());

    let index_head_offset = index_reader.current_head().expect("get index head offset");
    let index_tail_offset = index_reader.seek(SeekFrom::End(0)).expect("get index tail offset");
    if index_tail_offset < index_head_offset + IndexRecord::RECORD_SIZE as u64 {
        println!("Index has no records");
        return;
    }
    
    index_reader.seek(SeekFrom::Start(index_head_offset)).expect("seek to first index");
    let mut index_reader = index_reader.take(index_tail_offset - index_head_offset);
    
    let mut stream_begin_offset = u64::MAX;
    for _ in 0..opts.index_num {
        let index_record = index_record_reader.read(&mut index_reader).expect("read index");
        println!("{:?}", index_record);
        
        if opts.show_event {
            let stream_end_offset = index_record.offset;
            if stream_begin_offset < stream_end_offset {
                stream_reader.seek(SeekFrom::Start(stream_begin_offset)).expect("seek to stream begin offset");
                let mut reader = stream_reader.take(stream_end_offset - stream_begin_offset);
                loop {
                    let mut event_reader = EventReader::new();
                    let required_buffer_length =
                        match event_reader.read_required_buffer_length(&mut reader) {
                            Ok(n) => n,
                            Err(e) if e.kind() == ErrorKind::UnexpectedEof && reader.limit() == 0 => {
                                break;
                            },
                            Err(e) => {
                                println!("{:?}", e);
                                return;
                            },
                    };
                    let mut read_buffer: Vec<u8> = vec![0; required_buffer_length];
                    let event = event_reader.read_event(&mut reader, &mut read_buffer[..]).expect("read event");
                    println!("{:?}", event.header);
                }
                stream_reader = reader.into_inner();
            }
            stream_begin_offset = stream_end_offset;
        }
    }
}
