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

use anyhow::Error;
use gst_rtsp_server::prelude::*;
use gst_rtsp_server::{RTSPMediaFactory, RTSPServer};
use std::sync::mpsc;
use std::thread;
use tracing::{info, debug};

pub struct RTSPCameraSimulator {
    server: RTSPServer,
    port: Option<i32>,
    path: String,
    main_loop: glib::MainLoop,
}

impl RTSPCameraSimulator {
    #[allow(non_snake_case)]
    pub fn new(width: u64, height: u64, fps: u64, target_rate_KB_per_sec: f64) -> Result<RTSPCameraSimulator, Error> {
        let main_loop = glib::MainLoop::new(None, false);
        let server = RTSPServer::new();
        let mounts = server.get_mount_points().unwrap();
        let factory = RTSPMediaFactory::new();
        let target_rate_kbits_per_sec = (target_rate_KB_per_sec * 8.0) as u64;
        let pipeline_description = format!(
            "videotestsrc name=src is-live=true do-timestamp=true \
            ! video/x-raw,width={width},height={height},framerate={fps}/1 \
            ! videoconvert \
            ! clockoverlay font-desc=\"Sans, 48\" time-format=\"%F %T\" shaded-background=true \
            ! timeoverlay valignment=bottom font-desc=\"Sans, 48\" shaded-background=true \
            ! x264enc tune=zerolatency key-int-max=30 bitrate={target_rate_kbits_per_sec} \
            ! h264parse \
            ! rtph264pay name=pay0 pt=96",
            width = width,
            height = height,
            fps = fps,
            target_rate_kbits_per_sec = target_rate_kbits_per_sec,
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
            let port = server.get_bound_port();
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
