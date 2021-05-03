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
use glib::subclass::prelude::*;
use gst_rtsp::RTSPUrl;
use gst_rtsp_server::prelude::*;
use gst_rtsp_server::{RTSPMediaFactory, RTSPServer};
use gst_rtsp_server::subclass::prelude::*;
use std::sync::{Arc, Mutex};
use std::thread;
use tracing::info;

pub struct RTSPCameraSimulator {
    server: RTSPServer,
    port: Arc<Mutex<i32>>,
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
        info!(
            "RTSP server is configured for rtsp://{}:{}{}",
            server.get_address().unwrap(),
            server.get_bound_port(),
            path,
        );
        Ok(RTSPCameraSimulator {
            server,
            path: path.to_owned(),
            port: Arc::new(Mutex::new(0)),
            main_loop,
        })
    }

    pub fn get_url(&self) -> String {
        let port = self.port.lock().unwrap();
        assert!(*port > 0);
        format!("rtsp://localhost:{}{}", port, self.path)
    }

    pub fn start(&self) -> Result<(), Error>{
        let main_loop_clone = self.main_loop.clone();
        let server = self.server.clone();
        let port = self.port.clone();
        info!("RTSP server starting on port {}", server.get_bound_port());
        let _ = thread::spawn(move || {
            let source_id = server.attach(None).unwrap();
            info!("RTSP server started on port {}", server.get_bound_port());
            let mut port = port.lock().unwrap();
            *port = server.get_bound_port();
            drop(port);
            main_loop_clone.run();
            glib::source_remove(source_id);
            info!("RTSP server stopped");
        });
        // TODO: Need to wait for port to be set.
        std::thread::sleep(std::time::Duration::from_millis(1000));
        Ok(())
    }
}

impl Drop for RTSPCameraSimulator {
    fn drop(&mut self) {
        info!("RTSP server stopping");
        self.main_loop.quit();
    }
}
