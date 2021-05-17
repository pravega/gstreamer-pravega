//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

// Simulate an RTSP camera.
// Based on:
//   - https://gitlab.freedesktop.org/gstreamer/gstreamer-rs/-/blob/master/examples/src/bin/rtsp-server.rs
//   - https://gitlab.freedesktop.org/gstreamer/gstreamer-rs/-/blob/master/examples/src/bin/rtsp-server-record.rs
// See also /apps/src/bin/rtsp-camera-simulator.rs.

#![allow(dead_code)]

use anyhow::Error;
use derive_builder::*;
use gst_rtsp_server::prelude::*;
use gst_rtsp_server::{RTSPMediaFactory, RTSPServer};
use std::sync::mpsc;
use std::thread;
use tracing::{info, debug};

/// Start an in-process RTSP server that simulates a camera
/// or use an external RTSP server if specified in the RTSP_URL environment variable.
/// The in-process RTSP server will be stopped when the returned value is dropped.
pub fn start_or_get_rtsp_test_source(config: RTSPCameraSimulatorConfig) -> (String, Option<RTSPCameraSimulator>) {
    match std::env::var("RTSP_URL") {
        Ok(rtsp_url) if !rtsp_url.is_empty() => {
            info!("Using external RTSP server at {}", rtsp_url);
            (rtsp_url, None)
        },
        _ => {
            let mut rtsp_server = RTSPCameraSimulator::new(config).unwrap();
            rtsp_server.start().unwrap();
            let rtsp_url = rtsp_server.get_url().unwrap();
            info!("Using in-process RTSP camera simulator at {}", rtsp_url);
            (rtsp_url, Some(rtsp_server))
        }
    }
}

#[derive(Builder)]
pub struct RTSPCameraSimulatorConfig {
    #[builder(default = "640")]
    pub width: u64,
    #[builder(default = "480")]
    pub height: u64,
    #[builder(default = "20")]
    pub fps: u64,
    #[builder(default = "30")]
    pub key_frame_interval_max: u64,
    #[builder(default = "10.0")]
    pub target_rate_kilobytes_per_sec: f64,
    // Default tune ("zerolatency") does not use B-frames and is typical for RTSP cameras. Use "0" to use B-frames.
    #[builder(default = "\"/zerolatency\".to_owned()")]
    pub tune: String,
    #[builder(default = "\"/cam/realmonitor\".to_owned()")]
    pub path: String,
}

pub struct RTSPCameraSimulator {
    server: RTSPServer,
    port: Option<i32>,
    path: String,
    main_loop: glib::MainLoop,
}

impl RTSPCameraSimulator {
    pub fn new(config: RTSPCameraSimulatorConfig) -> Result<RTSPCameraSimulator, Error> {
        let main_loop = glib::MainLoop::new(None, false);
        let server = RTSPServer::new();
        let mounts = server.mount_points().unwrap();
        let factory = RTSPMediaFactory::new();
        let target_rate_kilobits_per_sec = (config.target_rate_kilobytes_per_sec * 8.0) as u64;
        let pipeline_description = format!(
            "videotestsrc name=src is-live=true do-timestamp=true \
            ! video/x-raw,width={width},height={height},framerate={fps}/1 \
            ! videoconvert \
            ! clockoverlay font-desc=\"Sans, 48\" time-format=\"%F %T\" shaded-background=true \
            ! timeoverlay valignment=bottom font-desc=\"Sans, 48\" shaded-background=true \
            ! x264enc bitrate={target_rate_kbits_per_sec} key-int-max={key_frame_interval_max} tune={tune} \
            ! h264parse \
            ! rtph264pay name=pay0 pt=96",
            width = config.width,
            height = config.height,
            fps = config.fps,
            target_rate_kbits_per_sec = target_rate_kilobits_per_sec,
            key_frame_interval_max = config.key_frame_interval_max,
            tune = config.tune,
        );
        info!("Launch Pipeline: {}", pipeline_description);
        factory.set_launch(&pipeline_description[..]);
        let path = "/cam/realmonitor";
        server.set_service("0");
        mounts.add_factory(path, &factory);
        Ok(RTSPCameraSimulator {
            server,
            path: path.to_owned(),
            port: None,
            main_loop,
        })
    }

    /// Get RTSP URL for this server.
    pub fn get_url(&self) -> Result<String, Error> {
        let port = self.port.expect("Port unknown. You must call start() before get_url().");
        assert!(port > 0);
        Ok(format!("rtsp://localhost:{}{}", port, self.path))
    }

    /// Start RTSP server in another thread.
    pub fn start(&mut self) -> Result<(), Error>{
        let main_loop_clone = self.main_loop.clone();
        let server = self.server.clone();
        let (tx, rx) = mpsc::channel();
        debug!("RTSP server starting");
        let _ = thread::spawn(move || {
            let source_id = server.attach(None).unwrap();
            // We can only get the bound port after server.attach().
            let port = server.bound_port();
            tx.send(port).unwrap();
            main_loop_clone.run();
            glib::source_remove(source_id);
            info!("RTSP server stopped");
        });
        let port = rx.recv().unwrap();
        self.port = Some(port);
        info!("RTSP server started on port {}", port);
        Ok(())
    }
}

impl Drop for RTSPCameraSimulator {
    fn drop(&mut self) {
        info!("RTSP server stopping");
        self.main_loop.quit();
    }
}
