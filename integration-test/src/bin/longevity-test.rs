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
// use gstreamer_video as gst_video;
// use gst_video::prelude::*;
use integration_test::utils::{run_pipeline_until_eos};
// use pravega_video::timestamp::PravegaTimestamp;
// use std::{convert::TryInto, os::raw::c_void, time::SystemTime};
// use std::process;
// use std::ops;
#[allow(unused_imports)]
use tracing::{error, warn, info, debug, trace, event, Level, span};
use tracing_subscriber::fmt::format::FmtSpan;

/// Default logging configuration for GStreamer and GStreamer plugins.
/// Valid levels are: none, ERROR, WARNING, FIXME, INFO, DEBUG, LOG, TRACE, MEMDUMP
/// See [https://gstreamer.freedesktop.org/documentation/tutorials/basic/debugging-tools.html?gi-language=c#the-debug-log].
pub const DEFAULT_GST_DEBUG: &str = "FIXME,pravegasrc:INFO";
/// Default logging configuration for for Rust tracing.
/// Valid levels are: error, warn, info, debug, trace
pub const DEFAULT_RUST_LOG: &str = "integration_test=info,warn";

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

    run_pipeline_until_eos(&pipeline)?;

    info!("main: END");
    Ok(())
}
