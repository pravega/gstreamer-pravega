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

use anyhow::Error;
use glib::subclass::prelude::*;
use gst_rtsp::RTSPUrl;
use gst_rtsp_server::prelude::*;
use gst_rtsp_server::{RTSPMediaFactory, RTSPServer};
use gst_rtsp_server::subclass::prelude::*;
use std::thread;
use tracing::info;

/// See also /apps/src/bin/rtsp-camera-simulator.rs.
pub struct RTSPCameraSimulator {
    main_loop: glib::MainLoop,
    server: RTSPServer,
}

impl RTSPCameraSimulator {
    pub fn new() -> Result<RTSPCameraSimulator, Error> {
        let main_loop = glib::MainLoop::new(None, false);
        let server = RTSPServer::new();
        let mounts = server.get_mount_points().unwrap();
        let factory = media_factory::Factory::default();
        let factory: RTSPMediaFactory = factory.dynamic_cast::<RTSPMediaFactory>().unwrap();
        let path = "/cam/realmonitor";
        mounts.add_factory(path, &factory);
        info!(
            "RTSP Camera Simulator is configured for rtsp://{}:{}{}",
            server.get_address().unwrap(),
            server.get_bound_port(),
            path,
        );
        Ok(RTSPCameraSimulator {
            main_loop,
            server
        })
    }

    pub fn start(&self) {
        let main_loop_clone = self.main_loop.clone();
        let server = self.server.clone();
        let _ = thread::spawn(move || {
            let source_id = server.attach(None).unwrap();
            info!("RTSP Camera Simulator started");
            main_loop_clone.run();
            glib::source_remove(source_id);
            info!("RTSP Camera Simulator stopped");
        });
    }
}

impl Drop for RTSPCameraSimulator {
    fn drop(&mut self) {
        info!("RTSP Camera Simulator stopping");
        self.main_loop.quit();
    }
}

mod media_factory {
    use super::*;

    mod imp {
        use super::*;

        pub struct Factory {}

        #[glib::object_subclass]
        impl ObjectSubclass for Factory {
            const NAME: &'static str = "RsRTSPMediaFactory";
            type Type = super::Factory;
            type ParentType = RTSPMediaFactory;

            fn new() -> Self {
                Self {}
            }
        }

        impl ObjectImpl for Factory {
        }

        impl RTSPMediaFactoryImpl for Factory {
            // This creates the GStreamer pipeline that will generate the video to send to the RTSP client.
            fn create_element(
                &self,
                _factory: &Self::Type,
                _url: &RTSPUrl,
            ) -> Option<gst::Element> {
                let pipeline_description =
                        "videotestsrc name=src is-live=true do-timestamp=true".to_owned()
                        + " ! " + &format!("video/x-raw,width={},height={},framerate=20/1", 640, 480)[..]
                        + " ! videoconvert"
                        + " ! clockoverlay font-desc=\"Sans, 48\" time-format=\"%F %T\" shaded-background=true"
                        + " ! timeoverlay valignment=bottom font-desc=\"Sans, 48\" shaded-background=true"
                        + " ! " + &format!("x264enc tune=zerolatency key-int-max=30 bitrate={}", 10)[..]
                        + " ! h264parse"
                        + " ! rtph264pay name=pay0 pt=96"
                    ;
                info!("RTSP Camera Simulator Launch Pipeline: {}", pipeline_description);
                let bin = gst::parse_launch(&pipeline_description.to_owned()).unwrap();
                Some(bin.upcast())
            }
        }
    }

    glib::wrapper! {
        pub struct Factory(ObjectSubclass<imp::Factory>) @extends RTSPMediaFactory;
    }

    unsafe impl Send for Factory {}
    unsafe impl Sync for Factory {}

    impl Default for Factory {
        fn default() -> Factory {
            glib::Object::new(&[]).expect("Failed to create factory")
        }
    }
}
