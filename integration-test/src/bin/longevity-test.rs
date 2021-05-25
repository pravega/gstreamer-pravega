//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

use anyhow::Error;
use clap::Clap;
use gst::prelude::*;
use gstpravega::utils::clocktime_to_pravega;
use integration_test::utils::{run_pipeline_until_eos};
use pravega_video::timestamp::{PravegaTimestamp, TimeDelta, MSECOND};
use std::sync::{Arc, Mutex};
#[allow(unused_imports)]
use tracing::{error, warn, info, debug, trace, event, Level, span};
use tracing_subscriber::fmt::format::FmtSpan;

/// Default logging configuration for GStreamer and GStreamer plugins.
/// Valid levels are: none, ERROR, WARNING, FIXME, INFO, DEBUG, LOG, TRACE, MEMDUMP
/// See [https://gstreamer.freedesktop.org/documentation/tutorials/basic/debugging-tools.html?gi-language=c#the-debug-log].
pub const DEFAULT_GST_DEBUG: &str = "WARN,pravegasrc:INFO,qtdemux:ERROR";
/// Default logging configuration for for Rust tracing.
/// Valid levels are: error, warn, info, debug, trace
pub const DEFAULT_RUST_LOG: &str = "longevity_test=debug,warn";

/// Pravega video player.
#[derive(Clap)]
struct Opts {
    /// Pravega controller in format "tcp://127.0.0.1:9090"
    #[clap(short, long, default_value = "tcp://127.0.0.1:9090")]
    controller: String,
    /// The filename containing the Keycloak credentials JSON. If missing or empty, authentication will be disabled.
    #[clap(short, long)]
    keycloak_file: Option<String>,
    /// Pravega scope/stream
    #[clap(short, long)]
    stream: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StreamingDecodedBufferValidator {
    stream: String,
    max_gap: TimeDelta,
    prev_pts: PravegaTimestamp,
    prev_pts_plus_duration: PravegaTimestamp,
    pts_is_none_count: u64,

}

impl StreamingDecodedBufferValidator {
    pub fn new(stream: &str, max_gap: TimeDelta) -> StreamingDecodedBufferValidator{
        StreamingDecodedBufferValidator {
            stream: stream.to_owned(),
            max_gap: max_gap,
            prev_pts: PravegaTimestamp::none(),
            prev_pts_plus_duration: PravegaTimestamp::none(),
            pts_is_none_count: 0,
        }
    }

    pub fn record_buffer(&mut self, buffer: &gst::BufferRef) {
        let flags = buffer.flags();
        let pts = clocktime_to_pravega(buffer.pts());
        if pts.is_none() {
            event!(Level::WARN, description = "PTS is none",
                prev_pts = ?self.prev_pts,
                offset = buffer.offset(), size = buffer.size(), flags = ?flags, stream = %self.stream);
            self.pts_is_none_count += 1;
        } else {
            if self.prev_pts.is_none() {
                self.prev_pts = pts;
            } else {
                let time_delta = pts - self.prev_pts;
                if time_delta >= 0 * MSECOND {
                    if time_delta > self.max_gap {
                        event!(Level::WARN, description = "Gap in PTS is too large",
                        time_delta = ?time_delta, prev_pts = ?self.prev_pts,
                        pts = ?pts, offset = buffer.offset(), size = buffer.size(), flags = ?flags, stream = %self.stream);
                    }
                    self.prev_pts = pts;
                } else {
                    event!(Level::WARN, description = "PTS is decreasing",
                        time_delta = ?time_delta, prev_pts = ?self.prev_pts,
                        pts = ?pts, offset = buffer.offset(), size = buffer.size(), flags = ?flags, stream = %self.stream);
                }
            }
        }
        if flags.contains(gst::BufferFlags::DISCONT) {
            event!(Level::WARN, description = "discontinuity",
                pts = ?pts, offset = buffer.offset(), size = buffer.size(), flags = ?flags, stream = %self.stream);
        }
        if flags.contains(gst::BufferFlags::CORRUPTED) {
            event!(Level::WARN, description = "corrupted",
                pts = ?pts, offset = buffer.offset(), size = buffer.size(), flags = ?flags, stream = %self.stream);
        }
    }

    pub fn log_stats(&self) {
        event!(Level::INFO, description = "statistics",
            pts_is_none_count = self.pts_is_none_count,
            pts = ?self.prev_pts, stream = %self.stream);
    }
}

fn main() -> Result<(), Error> {
    let opts: Opts = Opts::parse();

    let filter = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| DEFAULT_RUST_LOG.to_owned());
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_span_events(FmtSpan::CLOSE)
        .init();

    match std::env::var("GST_DEBUG") {
        Ok(_) => (),
        Err(_) => std::env::set_var("GST_DEBUG", DEFAULT_GST_DEBUG),
    };

    gst::init()?;
    gstpravega::plugin_register_static().unwrap();

    let pipeline_description = format!(
        "pravegasrc name=src \
          start-mode=earliest \
        ! decodebin \
        ! appsink name=sink sync=false"
    );
    info!("Launch Pipeline: {}", pipeline_description);
    let pipeline = gst::parse_launch(&pipeline_description.to_owned()).unwrap();
    let pipeline = pipeline.dynamic_cast::<gst::Pipeline>().unwrap();

    let pravegasrc = pipeline.clone().dynamic_cast::<gst::Pipeline>().unwrap().by_name("src").unwrap();
    pravegasrc.set_property("controller", &opts.controller).unwrap();
    pravegasrc.set_property("stream", &opts.stream).unwrap();
    pravegasrc.set_property("keycloak-file", &opts.keycloak_file.unwrap()).unwrap();
    pravegasrc.set_property("allow-create-scope", &false).unwrap();

    let max_gap = 100 * MSECOND;

    let validator = Arc::new(Mutex::new(
        StreamingDecodedBufferValidator::new(
            &opts.stream[..],
            max_gap,
    )));
    let validator_clone = validator.clone();

    // event!(Level::WARN, pts = "12345", description = "test");

    let timeout_id = glib::timeout_add(std::time::Duration::from_millis(4000), move || {
        info!("timeout");
        glib::Continue(true)
    });

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
                        let mut validator = validator_clone.lock().unwrap();
                        validator.record_buffer(buffer);
                        Ok(gst::FlowSuccess::Ok)
                    })
                    .build()
            );
        },
        None => warn!("Element named 'sink' not found"),
    };

    run_pipeline_until_eos(&pipeline)?;

    info!("main: END");
    Ok(())
}
