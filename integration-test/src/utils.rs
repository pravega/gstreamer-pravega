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
use derive_builder::*;
use gst::{BufferFlags, ClockTime};
use gst::prelude::*;
use gstpravega::utils::clocktime_to_pravega;
use pravega_client_config::ClientConfig;
use pravega_client::client_factory::ClientFactory;
use pravega_client_shared::{Scope, Stream, Segment, ScopedSegment};
use pravega_video::index::{IndexSearcher, SearchMethod, get_index_stream_name};
use pravega_video::timestamp::{PravegaTimestamp, TimeDelta, SECOND, NSECOND};
use std::convert::TryFrom;
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
    pub dts: PravegaTimestamp,
    pub duration: TimeDelta,
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
    }
}

impl From<&gst::BufferRef> for BufferSummary {
    fn from(buffer: &gst::BufferRef) -> BufferSummary {
        BufferSummary {
            pts: clocktime_to_pravega(buffer.pts()),
            dts: clocktime_to_pravega(buffer.dts()),
            duration: TimeDelta(buffer.duration().nanoseconds().map(|t| t as i128)),
            size: buffer.size() as u64,
            offset: buffer.offset(),
            offset_end: buffer.offset_end(),
            flags: buffer.flags(),
        }
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

    /// Returns list of DTSs that are not None.
    pub fn valid_dts(&self) -> Vec<PravegaTimestamp> {
        self.buffer_summary_list
            .iter()
            .map(|s| s.dts)
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

    /// Returns first DTS that is not None.
    pub fn first_valid_dts(&self) -> PravegaTimestamp {
        match self.valid_dts().first() {
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

    /// Returns minimum PTS.
    pub fn min_pts(&self) -> PravegaTimestamp {
        let t = self.buffer_summary_list
            .iter()
            .map(|s| s.pts)
            .filter(|c| c.is_some())
            .min();
        match t {
            Some(t) => t.to_owned(),
            None => PravegaTimestamp::none(),
        }
    }

    /// Returns maximum PTS + duration.
    pub fn max_pts_plus_duration(&self) -> PravegaTimestamp {
        let t = self.buffer_summary_list
            .iter()
            .map(|s| s.pts + s.duration.or_zero())
            .filter(|c| c.is_some())
            .max();
        match t {
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

    /// Returns timespan of buffers, including duration.
    pub fn pts_range(&self) -> TimeDelta {
        self.max_pts_plus_duration() - self.first_valid_pts()
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

    pub fn min_size(&self) -> u64 {
        self.buffer_summary_list
            .iter()
            .map(|s| s.size)
            .min()
            .unwrap_or_default()
    }

    pub fn max_size(&self) -> u64 {
        self.buffer_summary_list
            .iter()
            .map(|s| s.size)
            .max()
            .unwrap_or_default()
    }

    pub fn corrupted_buffer_count(&self) -> u64 {
        self.buffer_summary_list
            .iter()
            .filter(|s| s.flags.contains(gst::BufferFlags::CORRUPTED))
            .count() as u64
    }

    /// Check that pts[i] + duration[i] == pts[i+1].
    /// If duration is none in the buffer, then assume default_duration +/- 1 ns.
    pub fn imperfect_pts_count(&self, default_duration: TimeDelta) -> u64 {
        let mut prev_pts = PravegaTimestamp::none();
        let mut prev_duration = TimeDelta::none();
        self.buffer_summary_list
            .iter()
            .filter(|s| {
                // Allow PTS to differ by 1 ns from default_duration due to roundoff error.
                let duration_min = prev_duration.or(default_duration - 1 * NSECOND);
                let duration_max = prev_duration.or(default_duration + 1 * NSECOND);
                let pts_delta = s.pts - prev_pts;
                let imperfect = if pts_delta.is_some() && duration_min.is_some() && duration_max.is_some() {
                    if duration_min <= pts_delta && pts_delta <= duration_max {
                        false
                    } else {
                        warn!("imperfect_timestamp_count: prev_pts={}, prev_duration={}, pts_delta={}, pts={}",
                            prev_pts, prev_duration, pts_delta, s.pts);
                        true
                    }
                } else {
                    false
                };
                prev_pts = s.pts;
                prev_duration = s.duration;
                imperfect
            })
            .count() as u64
    }

    pub fn decreasing_pts_count(&self) -> u64 {
        let mut prev_pts = PravegaTimestamp::none();
        self.buffer_summary_list
            .iter()
            .filter(|s| {
                if s.pts.is_some() {
                    let decreasing = if prev_pts <= s.pts {
                        false
                    } else {
                        warn!("decreasing_pts_count: prev_pts={}, pts={}", prev_pts, s.pts);
                        true
                    };
                    prev_pts = s.pts;
                    decreasing
                } else {
                    false
                }
            })
            .count() as u64
    }

    pub fn decreasing_dts_count(&self) -> u64 {
        let mut prev_dts = PravegaTimestamp::none();
        self.buffer_summary_list
            .iter()
            .filter(|s| {
                if s.dts.is_some() {
                    let decreasing = if prev_dts <= s.dts {
                        false
                    } else {
                        warn!("decreasing_dts_count: prev_dts={}, dts={}", prev_dts, s.dts);
                        true
                    };
                    prev_dts = s.dts;
                    decreasing
                } else {
                    false
                }
            })
            .count() as u64
    }

    pub fn dump(&self, prefix: &str) {
        let mut prev_pts = PravegaTimestamp::none();
        for (i, s) in self.buffer_summary_list.iter().enumerate() {
            let pts_delta = s.pts - prev_pts;
            prev_pts = s.pts;
            debug!("{}{:5}: {:?}, pts_delta: {}", prefix, i, s, pts_delta);
        }
    }

    pub fn dump_timestamps(&self, prefix: &str) {
        let mut prev_pts = PravegaTimestamp::none();
        for (i, s) in self.buffer_summary_list.iter().enumerate() {
            let pts_delta = s.pts - prev_pts;
            prev_pts = s.pts;
            debug!("{}{:5}: pts: {:?}, dts: {}, duration: {}, pts_delta: {}", prefix, i, s.pts, s.dts, s.duration, pts_delta);
        }
    }
}

impl fmt::Display for BufferListSummary {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.write_fmt(format_args!("BufferListSummary {{ num_buffers: {}, num_buffers_with_valid_pts: {}, \
            first_pts: {}, first_valid_pts: {}, first_valid_dts: {}, min_pts: {}, last_valid_pts: {}, max_pts_plus_duration: {}, pts_range: {}, \
            min_size: {}, max_size: {} }}",
            self.num_buffers(), self.num_buffers_with_valid_pts(),
            self.first_pts(), self.first_valid_pts(), self.first_valid_dts(), self.min_pts(),
            self.last_valid_pts(), self.max_pts_plus_duration(), self.pts_range(),
            self.min_size(), self.max_size()))
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
    let sink = pipeline.by_name("sink");
    match sink {
        Some(sink) => {
            let sink = sink.downcast::<gst_app::AppSink>().unwrap();
            sink.set_callbacks(
                gst_app::AppSinkCallbacks::builder()
                    .new_sample(move |sink| {
                        let sample = sink.pull_sample().unwrap();
                        trace!("sample={:?}", sample);
                        let buffer = sample.buffer().unwrap();
                        let summary = BufferSummary::from(buffer);
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

pub fn run_pipeline_until_eos(pipeline: &gst::Pipeline) -> Result<(), Error> {
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
                let (_, property_name, value) = p.get();
                match value {
                    Some(value) => match value.get::<String>() {
                        Ok(value) => if !value.is_empty() {
                            debug!("PropertyNotify: {}={}", property_name, value);
                        },
                        _ => {}
                    },
                    _ => (),
                };
            },
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
    let runtime = client_factory.runtime();
    let scoped_segment = ScopedSegment {
        scope: scope.clone(),
        stream: stream.clone(),
        segment: Segment::from(0),
    };
    let writer = client_factory.create_byte_writer(scoped_segment);
    let index_scoped_segment = ScopedSegment {
        scope: scope.clone(),
        stream: index_stream.clone(),
        segment: Segment::from(0),
    };
    let index_writer = client_factory.create_byte_writer(index_scoped_segment.clone());
    let index_reader = client_factory.create_byte_reader(index_scoped_segment.clone());
    let mut index_searcher = IndexSearcher::new(index_reader);
    let index_record = index_searcher.search_timestamp_and_return_index_offset(
        truncate_before_timestamp, SearchMethod::Before).unwrap();
        info!("Truncating prior to {:?}", index_record);
    runtime.block_on(index_writer.truncate_data_before(index_record.1 as i64)).unwrap();
    info!("Index truncated at offset {}", index_record.1);
    runtime.block_on(writer.truncate_data_before(index_record.0.offset as i64)).unwrap();
    info!("Data truncated at offset {}", index_record.0.offset);
}

#[derive(Builder, Debug, Clone)]
pub struct VideoTestSrcConfig {
    #[builder(default = "640")]
    pub width: u64,
    #[builder(default = "480")]
    pub height: u64,
    #[builder(default = "30")]
    pub fps: u64,
    #[builder(default = "\"2001-02-03T04:00:00.000Z\".to_owned()")]
    pub first_utc: String,
    #[builder(default = "10 * pravega_video::timestamp::SECOND")]
    pub duration: TimeDelta,
}

impl VideoTestSrcConfig {
    pub fn pipeline(&self) -> String {
        let first_timestamp = PravegaTimestamp::try_from(Some(&self.first_utc[..])).unwrap();
        let num_buffers = (self.fps * self.duration / SECOND).unwrap();
        format!("\
            videotestsrc name=src \
              timestamp-offset={first_timestamp} \
              num-buffers={num_buffers} \
            ! video/x-raw,width={width},height={height},framerate={fps}/1 \
            ! videoconvert \
            ! timeoverlay valignment=bottom font-desc=\"Sans 48px\" \
            ! videoconvert",
            first_timestamp = first_timestamp.nanoseconds().unwrap(),
            num_buffers = num_buffers,
            width = self.width,
            height = self.height,
            fps = self.fps,
        )
    }
}

#[derive(Debug, Clone)]
pub enum VideoSource {
    VideoTestSrc(VideoTestSrcConfig),
}

impl VideoSource {
    pub fn pipeline(&self) -> String {
        match self {
            VideoSource::VideoTestSrc(config) => config.pipeline(),
        }
    }
}

#[derive(Builder, Clone, Debug)]
pub struct H264EncoderConfig {
    #[builder(default = "250.0")]
    pub bitrate_kilobytes_per_sec: f64,
    /// Number of frames between key frames.
    #[builder(default = "0")]
    pub key_int_max_frames: u32,
    /// Default tune ("zerolatency") does not use B-frames and is typical for RTSP cameras. Use "0" to use B-frames.
    #[builder(default = "\"zerolatency\".to_owned()")]
    pub tune: String,
}

impl H264EncoderConfig {
    pub fn pipeline(&self) -> String {
        format!("x264enc bitrate={bitrate_kilobits_per_sec} key-int-max={key_int_max_frames} tune={tune}",
            bitrate_kilobits_per_sec = (self.bitrate_kilobytes_per_sec * 8.0) as u32,
            key_int_max_frames = self.key_int_max_frames,
            tune = self.tune,
        )
    }
}

#[derive(Debug, Clone)]
pub enum VideoEncoder {
    H264(H264EncoderConfig),
}

impl VideoEncoder {
    pub fn pipeline(&self) -> String {
        match self {
            VideoEncoder::H264(config) => config.pipeline(),
        }
    }
}

#[derive(Builder, Clone, Debug)]
pub struct Mp4MuxConfig {
    #[builder(default = "100 * pravega_video::timestamp::MSECOND")]
    fragment_duration: TimeDelta,
}

impl Mp4MuxConfig {
    pub fn pipeline(&self) -> String {
        format!("\
            mp4mux streamable=true fragment-duration={fragment_duration} \
            ! identity name=mp4mux_ silent=false \
            ! fragmp4pay \
            ! identity name=fragmp4 silent=false \
            ",
            fragment_duration = self.fragment_duration.milliseconds().unwrap())
    }
}

#[derive(Debug, Clone)]
pub enum ContainerFormat {
    // MPEG transport stream
    MpegTs,
    // Fragmented MP4, MPEG-4 Part 14, QuickTime, ISO/IEC 14496-14:2003, ISO Base Media File Format
    Mp4(Mp4MuxConfig),
}

impl ContainerFormat {
    pub fn pipeline(&self) -> String {
        match self {
            ContainerFormat::Mp4(config) => config.pipeline(),
            ContainerFormat::MpegTs => format!("mpegtsmux"),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// This function can be used to check an MP4 file for correctness.
    #[test]
    fn test_mp4_file_check_ignore() {
        gst_init();
        let location = "test.mp4";
        let fps = 20.0;
        let default_duration = (1e9 / fps) as u64 * NSECOND;

        let pipeline_description = format!(
            "filesrc location={location} \
            ! qtdemux \
            ! appsink name=sink sync=false",
            location = location,
        );
        let summary_demuxed = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        summary_demuxed.dump("summary_demuxed: ");
        debug!("summary_demuxed={}", summary_demuxed);
        assert_between_u64("decreasing_dts_count", summary_demuxed.decreasing_dts_count(), 0, 10000);
        assert_between_u64("decreasing_pts_count", summary_demuxed.decreasing_pts_count(), 0, 10000);
        assert_between_u64("corrupted_buffer_count", summary_demuxed.corrupted_buffer_count(), 0, 10000);
        assert_between_u64("imperfect_timestamp_count", summary_demuxed.imperfect_pts_count(default_duration), 0, 10000);

        let pipeline_description = format!(
            "filesrc location={location} \
            ! decodebin \
            ! appsink name=sink sync=false",
            location = location,
        );
        let summary_decoded = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        summary_decoded.dump("summary: ");
        debug!("summary_decoded={}", summary_decoded);
        assert_between_u64("decreasing_dts_count", summary_decoded.decreasing_dts_count(), 0, 10000);
        assert_between_u64("decreasing_pts_count", summary_demuxed.decreasing_pts_count(), 0, 10000);
        assert_between_u64("corrupted_buffer_count", summary_decoded.corrupted_buffer_count(), 0, 10000);
        assert_between_u64("imperfect_timestamp_count", summary_decoded.imperfect_pts_count(default_duration), 0, 10000);
    }

    /// This function can be used to check an RTSP file (created with rtsp-camera-to-file-gdp.sh) for correctness.
    #[test]
    fn test_rtsp_file_check_ignore() {
        gst_init();
        let location = "rtsp.gdp";
        let pipeline_description = format!(
            "filesrc location={location} \
            ! gdpdepay \
            ! rtph264depay \
            ! h264parse \
            ! video/x-h264,alignment=au \
            ! appsink name=sink sync=false",
            location = location,
        );
        let summary_gdpdepay = launch_pipeline_and_get_summary(&pipeline_description).unwrap();
        summary_gdpdepay.dump("summary_gdpdepay: ");
        debug!("summary_gdpdepay={}", summary_gdpdepay);
        assert_between_u64("decreasing_dts_count", summary_gdpdepay.decreasing_dts_count(), 0, 10000);
        assert_between_u64("decreasing_pts_count", summary_gdpdepay.decreasing_pts_count(), 0, 10000);
        assert_between_u64("corrupted_buffer_count", summary_gdpdepay.corrupted_buffer_count(), 0, 10000);
    }
}
