//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

#![allow(dead_code)]

use anyhow::{anyhow, Error};
use gst::{BufferFlags, ClockTime};
use gst::prelude::*;
use gstpravega::utils::clocktime_to_pravega;
use pravega_client_config::ClientConfig;
use pravega_client::client_factory::ClientFactory;
use pravega_client_shared::{Scope, Stream, Segment, ScopedSegment};
use pravega_video::index::{IndexSearcher, SearchMethod, get_index_stream_name};
use pravega_video::timestamp::{PravegaTimestamp, TimeDelta};
use std::fmt;
use std::sync::{Arc, Mutex};
#[allow(unused_imports)]
use tracing::{error, warn, info, debug, trace};
use crate::DEFAULT_GST_DEBUG;

/// Initialize GStreamer.
/// See log levels: https://gstreamer.freedesktop.org/documentation/tutorials/basic/debugging-tools.html?gi-language=c#the-debug-log
pub fn gst_init() {
    match std::env::var("GST_DEBUG") {
        Ok(_) => (),
        Err(_) => std::env::set_var("GST_DEBUG", DEFAULT_GST_DEBUG),
    };
    info!("GST_DEBUG={}", std::env::var("GST_DEBUG").unwrap_or_default());
    gst::init().unwrap();
    gstpravega::plugin_register_static().unwrap();
}

// TODO: Also compare hash of buffer contents.
#[derive(Clone, Debug)]
pub struct BufferSummary {
    pub pts: PravegaTimestamp,
    pub size: u64,
    /// Not used for equality.
    pub offset: u64,
    /// Not used for equality.
    pub offset_end: u64,
    pub flags: BufferFlags,
}

/// Compare BufferSummary to ensure that significant fields are equal.
impl PartialEq for BufferSummary {
    fn eq(&self, other: &Self) -> bool {
        self.pts == other.pts &&
            self.size == other.size &&
            self.flags.contains(gst::BufferFlags::DELTA_UNIT) == other.flags.contains(gst::BufferFlags::DELTA_UNIT)
            // TODO: Also compare DISCONT flag but first event from mp4mux has inconsistent DISCONT flag.
            // self.flags.contains(gst::BufferFlags::DISCONT) == other.flags.contains(gst::BufferFlags::DISCONT)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BufferListSummary {
    pub buffer_summary_list: Vec<BufferSummary>,
}

impl BufferListSummary {
    /// Returns list of PTSs.
    pub fn pts(&self) -> Vec<PravegaTimestamp> {
        self.buffer_summary_list
            .iter()
            .map(|s| s.pts)
            .collect()
    }

    /// Returns PTS of first buffer.
    pub fn first_pts(&self) -> PravegaTimestamp {
        match self.buffer_summary_list.first() {
            Some(s) => s.pts,
            None => PravegaTimestamp::none(),
        }
    }

    /// Returns PTS of last buffer.
    pub fn last_pts(&self) -> PravegaTimestamp {
        match self.buffer_summary_list.last() {
            Some(s) => s.pts,
            None => PravegaTimestamp::none(),
        }
    }

    /// Returns list of PTSs that are not None.
    pub fn valid_pts(&self) -> Vec<PravegaTimestamp> {
        self.buffer_summary_list
            .iter()
            .map(|s| s.pts)
            .filter(|c| c.is_some())
            .collect()
    }

    /// Returns first PTS that is not None.
    pub fn first_valid_pts(&self) -> PravegaTimestamp {
        match self.valid_pts().first() {
            Some(t) => t.to_owned(),
            None => PravegaTimestamp::none(),
        }
    }

    /// Returns last PTS that is not None.
    pub fn last_valid_pts(&self) -> PravegaTimestamp {
        match self.valid_pts().last() {
            Some(t) => t.to_owned(),
            None => PravegaTimestamp::none(),
        }
    }

    /// Returns first buffer with PTS after given PTS.
    pub fn first_buffer_after(&self, pts: PravegaTimestamp) -> Option<BufferSummary> {
        self.buffer_summary_list
            .iter()
            .filter(|s| s.pts > pts)
            .next()
            .cloned()
    }

    /// Returns buffers with PTS in given range.
    pub fn buffers_between(&self, min_pts: PravegaTimestamp, max_pts: PravegaTimestamp) -> Vec<BufferSummary> {
        self.buffer_summary_list
            .iter()
            .filter(|s| min_pts <= s.pts && s.pts <= max_pts)
            .cloned()
            .collect()
    }

    pub fn pts_range(&self) -> TimeDelta {
        self.last_valid_pts() - self.first_valid_pts()
    }

    /// Returns list of PTSs of all non-delta frames.
    pub fn non_delta_pts(&self) -> Vec<PravegaTimestamp> {
        self.buffer_summary_list
            .iter()
            .filter(|s| s.pts.is_some())
            .filter(|s| !s.flags.contains(gst::BufferFlags::DELTA_UNIT))
            .map(|s|s.pts)
            .collect()
    }

    pub fn num_buffers(&self) -> u64 {
        self.buffer_summary_list.len() as u64
    }

    pub fn num_buffers_with_valid_pts(&self) -> u64 {
        self.buffer_summary_list
            .iter()
            .map(|s| s.pts)
            .filter(|c| c.is_some())
            .count() as u64
    }

    pub fn dump(&self, prefix: &str) {
        for (i, s) in self.buffer_summary_list.iter().enumerate() {
            debug!("{}{:5}: {:?}", prefix, i, s);
        }
    }
}

impl fmt::Display for BufferListSummary {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.write_fmt(format_args!("BufferListSummary {{ num_buffers: {}, num_buffers_with_valid_pts: {}, \
            first_pts: {}, first_valid_pts: {}, last_valid_pts: {}, pts_range: {} }}",
            self.num_buffers(), self.num_buffers_with_valid_pts(),
            self.first_pts(), self.first_valid_pts(), self.last_valid_pts(), self.pts_range()))
    }
}

pub fn assert_between_clocktime(name: &str, actual: ClockTime, expected_min: ClockTime, expected_max: ClockTime) {
    debug!("{}: Actual:   {}    {}", name, actual, actual);
    debug!("{}: Expected: {} to {}", name, expected_min, expected_max);
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

pub fn assert_between_timestamp(name: &str, actual: PravegaTimestamp, expected_min: PravegaTimestamp, expected_max: PravegaTimestamp) {
    debug!("{}: Actual:   {:?}    {:?}", name, actual, actual);
    debug!("{}: Expected: {:?} to {:?}", name, expected_min, expected_max);
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

pub fn assert_timestamp_eq(name: &str, actual: PravegaTimestamp, expected: PravegaTimestamp) {
    debug!("{}: Actual:   {:?}", name, actual);
    debug!("{}: Expected: {:?}", name, expected);
    if actual.nanoseconds().is_none() {
        panic!("{} is None", name);
    }
    if actual != expected {
        panic!("{}: actual value {} is not equal to expected value {}", name, actual, expected);
    }
}

pub fn assert_timestamp_approx_eq(name: &str, actual: PravegaTimestamp, expected: PravegaTimestamp, lower_margin: TimeDelta, upper_margin: TimeDelta) {
    assert_between_timestamp(name, actual, expected - lower_margin, expected + upper_margin)
}

pub fn assert_between_u64(name: &str, actual: u64, expected_min: u64, expected_max: u64) {
    debug!("{}: Actual:   {}    {}", name, actual, actual);
    debug!("{}: Expected: {} to {}", name, expected_min, expected_max);
    if actual < expected_min {
        panic!("{}: actual value {} is less than expected minimum {}", name, actual, expected_min);
    }
    if actual > expected_max {
        panic!("{}: actual value {} is greater than expected maximum {}", name, actual, expected_max);
    }
}

pub fn launch_pipeline(pipeline_description: &str) -> Result<(), Error> {
    info!("Launch Pipeline: {}", pipeline_description);
    let pipeline = gst::parse_launch(&pipeline_description)?;
    let pipeline = pipeline.dynamic_cast::<gst::Pipeline>().unwrap();
    run_pipeline_until_eos(&pipeline)
}

/// Run a pipeline until end-of-stream and return a summary of buffers sent to the AppSink named 'sink'.
pub fn launch_pipeline_and_get_summary(pipeline_description: &str) -> Result<BufferListSummary, Error> {
    info!("Launch Pipeline: {}", pipeline_description);
    let pipeline = gst::parse_launch(&pipeline_description)?;
    let pipeline = pipeline.dynamic_cast::<gst::Pipeline>().unwrap();
    // Subscribe to any property changes.
    // Identity elements with silent=false will produce bus messages and will be logged by monitor_pipeline_until_eos.
    let _ = pipeline.add_property_deep_notify_watch(None, true);
    let summary_list = Arc::new(Mutex::new(Vec::new()));
    let summary_list_clone = summary_list.clone();
    let sink = pipeline
        .get_by_name("sink");
    match sink {
        Some(sink) => {
            let sink = sink.downcast::<gst_app::AppSink>().unwrap();
            sink.set_callbacks(
                gst_app::AppSinkCallbacks::builder()
                    .new_sample(move |sink| {
                        let sample = sink.pull_sample().unwrap();
                        trace!("sample={:?}", sample);
                        let buffer = sample.buffer().unwrap();
                        let summary = BufferSummary {
                            pts: clocktime_to_pravega(buffer.pts()),
                            size: buffer.size() as u64,
                            offset: buffer.offset(),
                            offset_end: buffer.offset_end(),
                            flags: buffer.flags(),
                        };
                        let mut summary_list = summary_list_clone.lock().unwrap();
                        summary_list.push(summary);
                        Ok(gst::FlowSuccess::Ok)
                    })
                    .build()
            );
        },
        None => warn!("Element named 'sink' not found"),
    };
    run_pipeline_until_eos(&pipeline)?;
    let summary_list = summary_list.lock().unwrap().clone();
    let summary = BufferListSummary {
        buffer_summary_list: summary_list,
    };
    Ok(summary)
}

fn run_pipeline_until_eos(pipeline: &gst::Pipeline) -> Result<(), Error> {
    pipeline.set_state(gst::State::Playing)?;
    monitor_pipeline_until_eos(pipeline)?;
    pipeline.set_state(gst::State::Null)?;
    Ok(())
}

pub fn monitor_pipeline_until_eos(pipeline: &gst::Pipeline) -> Result<(), Error> {
    let bus = pipeline.bus().unwrap();
    while let Some(msg) = bus.timed_pop(gst::CLOCK_TIME_NONE) {
        trace!("Bus message: {:?}", msg);
        match msg.view() {
            gst::MessageView::Eos(..) => break,
            gst::MessageView::Error(err) => {
                let msg = format!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
                let _ = pipeline.set_state(gst::State::Null);
                return Err(anyhow!(msg));
            },
            gst::MessageView::PropertyNotify(p) => {
                // Identity elements with silent=false will produce this message after watching with `pipeline.add_property_deep_notify_watch(None, true)`.
                debug!("{:?}", p);
            }
            _ => (),
        }
    }
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
