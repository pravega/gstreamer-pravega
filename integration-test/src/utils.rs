//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

use anyhow::{anyhow, Error};
use gst::ClockTime;
use gst::prelude::*;
use pravega_client_config::ClientConfig;
use pravega_client::client_factory::ClientFactory;
use pravega_client_shared::{Scope, Stream, Segment, ScopedSegment};
use pravega_video::index::{IndexSearcher, SearchMethod, get_index_stream_name};
use pravega_video::timestamp::PravegaTimestamp;
use std::sync::{Arc, Mutex};
// use std::convert::TryFrom;
use tracing::{error, info, debug};

pub fn assert_between(name: &str, actual: ClockTime, expected_min: ClockTime, expected_max: ClockTime) {
    if !actual.nanoseconds().is_some() {
        panic!("{} is None", name);
    }
    if expected_min.nanoseconds().is_some() && actual.nanoseconds().unwrap() < expected_min.nanoseconds().unwrap() {
        panic!("{}: actual value {} is less than expected minimum {}", name, actual, expected_min);
    }
    if expected_max.nanoseconds().is_some() && actual.nanoseconds().unwrap() > expected_max.nanoseconds().unwrap() {
        panic!("{}: actual value {} is greater than expected maximum {}", name, actual, expected_max);
    }
}

pub fn launch_pipeline(pipeline_description: String) -> Result<(), Error> {
    info!("Launch Pipeline: {}", pipeline_description);
    let pipeline = gst::parse_launch(&pipeline_description)?;
    let pipeline = pipeline.dynamic_cast::<gst::Pipeline>().unwrap();
    run_pipeline_until_eos(pipeline)
}

pub fn launch_pipeline_and_get_pts(pipeline_description: String) -> Result<Vec<ClockTime>, Error> {
    info!("Launch Pipeline: {}", pipeline_description);
    let pipeline = gst::parse_launch(&pipeline_description)?;
    let pipeline = pipeline.dynamic_cast::<gst::Pipeline>().unwrap();
    let sink = pipeline
        .get_by_name("sink")
        .unwrap()
        .downcast::<gst_app::AppSink>()
        .unwrap();
    let read_pts = Arc::new(Mutex::new(Vec::new()));
    let read_pts_clone = read_pts.clone();
    sink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |sink| {
                let sample = sink.pull_sample().unwrap();
                debug!("sample={:?}", sample);
                let pts = sample.get_buffer().unwrap().get_pts();
                debug!("pts={}", pts);
                let mut read_timestamps = read_pts_clone.lock().unwrap();
                read_timestamps.push(pts);
                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );
    run_pipeline_until_eos(pipeline)?;
    let read_pts = read_pts.lock().unwrap().clone();
    Ok(read_pts)
}

fn run_pipeline_until_eos(pipeline: gst::Pipeline) -> Result<(), Error> {
    pipeline.set_state(gst::State::Playing)?;
    let bus = pipeline.get_bus().unwrap();
    while let Some(msg) = bus.timed_pop(gst::CLOCK_TIME_NONE) {
        match msg.view() {
            gst::MessageView::Eos(..) => break,
            gst::MessageView::Error(err) => {
                let msg = format!(
                    "Error from {:?}: {} ({:?})",
                    err.get_src().map(|s| s.get_path_string()),
                    err.get_error(),
                    err.get_debug()
                );
                let _ = pipeline.set_state(gst::State::Null);
                return Err(anyhow!(msg));
            },
            _ => (),
        }
    }
    pipeline.set_state(gst::State::Null)?;
    Ok(())
}

pub fn truncate_stream(client_config: ClientConfig, scope_name: String, stream_name: String, truncate_before_timestamp: PravegaTimestamp) {
    info!("Truncating stream {}/{} before {}", scope_name, stream_name, truncate_before_timestamp);
    let index_stream_name = get_index_stream_name(&stream_name);
    let scope = Scope::from(scope_name);
    let stream = Stream::from(stream_name);
    let index_stream = Stream::from(index_stream_name);
    let client_factory = ClientFactory::new(client_config);
    let runtime = client_factory.get_runtime();
    let scoped_segment = ScopedSegment {
        scope: scope.clone(),
        stream: stream.clone(),
        segment: Segment::from(0),
    };
    let writer = client_factory.create_byte_stream_writer(scoped_segment);
    let index_scoped_segment = ScopedSegment {
        scope: scope.clone(),
        stream: index_stream.clone(),
        segment: Segment::from(0),
    };
    let index_writer = client_factory.create_byte_stream_writer(index_scoped_segment.clone());
    let index_reader = client_factory.create_byte_stream_reader(index_scoped_segment.clone());
    let mut index_searcher = IndexSearcher::new(index_reader);
    let index_record = index_searcher.search_timestamp_and_return_index_offset(
        truncate_before_timestamp, SearchMethod::Before).unwrap();
        info!("Truncating prior to {:?}", index_record);
    runtime.block_on(index_writer.truncate_data_before(index_record.1 as i64)).unwrap();
    info!("Index truncated at offset {}", index_record.1);
    runtime.block_on(writer.truncate_data_before(index_record.0.offset as i64)).unwrap();
    info!("Data truncated at offset {}", index_record.0.offset);
}
