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
use gst::prelude::*;
use log::info;
use std::path::Path;
use gst_rtsp_server::gio::{TlsFileDatabase};

/// RTSP Camera to Pravega.
#[derive(Clap)]
struct Opts {
    /// RTSP URL
    #[clap(long)]
    location: String,
    /// Tls ca file for secure connection. Tls will be disabled if not specified.
    #[clap(long, env = "TLS_CA_FILE")]
    tls_ca_file: Option<String>,
}

fn main() {
    env_logger::init();
    let opts: Opts = Opts::parse();

    // Initialize GStreamer
    if let Err(err) = gst::init() {
        eprintln!("Failed to initialize Gst: {}", err);
        return;
    }

    let pipeline_description =
        "rtspsrc name=src".to_owned()
        + "   buffer-mode=none"                 // Outgoing timestamps are calculated directly from the RTP timestamps. This mode is good for recording.
                                                // This will provide the RTP timestamps as PTS (and the arrival timestamps as DTS).
                                                // See https://gitlab.freedesktop.org/gstreamer/gst-plugins-base/issues/255
        + "   drop-messages-interval=0"         // Always log when rtp packets have been dropped
        + "   drop-on-latency=true"             // Drop oldest buffers when the queue is completely filled
        + "   latency=2000"                     // Set the maximum latency of the jitterbuffer (milliseconds).
                                                // Packets will be kept in the buffer for at most this time.
        + "   ntp-sync=true"                    // Required to get NTP timestamps as PTS
        + "   ntp-time-source=running-time"     // Required to get NTP timestamps as PTS
        //+ "   tls-validation-flags=generic-error"
        + " ! rtph264depay"                     // Extract H264 elementary stream
        + " ! h264parse"                        // Parse H264
        + " ! video/x-h264,alignment=au"        // Must align on Access Units for mpegtsmux
        + " ! avdec_h264"                     
        + " ! autovideosink sync=false"
        ;
    info!("Launch Pipeline: {}", pipeline_description);
    let pipeline = gst::parse_launch(&pipeline_description.to_owned()).unwrap();
    let pipeline = pipeline.dynamic_cast::<gst::Pipeline>().unwrap();

    let clock = gst::SystemClock::obtain();
    clock.set_property("clock-type", &gst::ClockType::Realtime).unwrap();
    println!("clock={:?}, time={:?}", clock, clock.time());
    pipeline.use_clock(Some(&clock));

    let rtspsrc = pipeline
        .clone()
        .dynamic_cast::<gst::Pipeline>().unwrap()
        .by_name("src").unwrap();
    rtspsrc.set_property("location", &opts.location).unwrap();

    if let Some(ca_file) = opts.tls_ca_file {
        let ca_path = Path::new(&ca_file);
        let ca_database = TlsFileDatabase::new(ca_path).expect("Failed to open tls ca certificate");
        rtspsrc.set_property("tls-database", ca_database).unwrap();
    }

    // Start playing
    pipeline
        .set_state(gst::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state");

    // Wait until error or EOS
    let bus = pipeline.bus().unwrap();
    for msg in bus.iter_timed(gst::CLOCK_TIME_NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                println!(
                    "Error from {:?}: {} ({:?})",
                    err.src().map(|s| s.path_string()),
                    err.error(),
                    err.debug()
                );
                break;
            }
            _ => (),
        }
    }

    // Shutdown pipeline
    pipeline
        .set_state(gst::State::Null)
        .expect("Unable to set the pipeline to the `Null` state");
}
