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
use clap::Clap;
use derive_more::{Display, Error};
use glib::subclass::prelude::*;
use glib::translate::*;
use gst::prelude::*;
use gst_rtsp_server::prelude::*;
use gst_rtsp_server::{RTSPAuth, RTSPToken};
use gst_rtsp_server::subclass::prelude::*;
use gst_rtsp_server::gio::{TlsCertificate};
use std::ptr;
use std::collections::HashMap;
#[allow(unused_imports)]
use tracing::{error, warn, info, debug, trace, event, Level, span};
use tracing_subscriber::fmt::format::FmtSpan;
use url::Url;
use std::path::Path;

/// Default logging configuration for GStreamer and GStreamer plugins.
/// Valid levels are: none, ERROR, WARNING, FIXME, INFO, DEBUG, LOG, TRACE, MEMDUMP
/// See [https://gstreamer.freedesktop.org/documentation/tutorials/basic/debugging-tools.html?gi-language=c#the-debug-log].
pub const DEFAULT_GST_DEBUG: &str = "FIXME";
/// Default logging configuration for for Rust tracing.
/// Valid levels are: error, warn, info, debug, trace
pub const DEFAULT_RUST_LOG: &str = "info";

#[derive(Debug, Display, Error)]
#[display(fmt = "Could not get mount points")]
struct NoMountPoints;

/// RTSP camera simulator
#[derive(Clap, Debug)]
struct Opts {
    /// TCP port to listen on
    #[clap(long, env = "CAMERA_PORT", default_value = "8554")]
    port: u16,
    /// URL path to accept
    #[clap(long, env = "CAMERA_PATH", default_value = "/cam/realmonitor")]
    path: String,
    /// User name for basic authentication
    #[clap(long, env = "CAMERA_USER", default_value = "user")]
    user_name: String,
    /// Password for basic authentication. Authentication will be disabled if not specified.
    #[clap(long, env = "CAMERA_PASSWORD")]
    password: Option<String>,
    /// Tls cert file for secure connection. Tls will be disabled if not specified.
    #[clap(long, env = "TLS_CERT_FILE")]
    tls_cert_file: Option<String>,
    /// Tls key file for secure connection. Tls will be disabled if not specified.
    #[clap(long, env = "TLS_KEY_FILE")]
    tls_key_file: Option<String>,
    /// Default width
    #[clap(long, env = "CAMERA_WIDTH", default_value = "320")]
    width: u32,
    /// Default height
    #[clap(long, env = "CAMERA_HEIGHT", default_value = "180")]
    height: u32,
    /// Default frames per second
    #[clap(long, env = "CAMERA_FPS", default_value = "30")]
    fps: f64,
    /// Default maximum key frame interval, in number of frames
    #[clap(long, env = "CAMERA_KEY_FRAME_INTERVAL_MAX", default_value = "30")]
    key_frame_interval_max: u32,
    /// Default target rate in KB/sec
    #[clap(long, env = "CAMERA_TARGET_RATE_KILOBYTES_PER_SEC", default_value = "25.0")]
    target_rate_kilobytes_per_sec: f64,
    /// If 0, hides the clock by default
    #[clap(long, env = "CAMERA_SHOW_CLOCK", default_value = "1")]
    show_clock: u8,
    /// If 1, the first connected client will start the pipeline and all subsequent clients using
    /// the same URL will get the same stream.
    /// This is useful for performance testing to reduce the CPU load.
    /// This causes a 5 second interruption in the stream when new clients connect.
    /// If 0, each client will use its own pipeline.
    #[clap(long, env = "SHARE_PIPELINE", default_value = "1")]
    share_pipeline: u8,
    /// Can be x264enc or nvv4l2h264enc
    #[clap(long, env = "VIDEO_ENCODER_PIPELINE", default_value = "x264enc")]
    video_encoder_pipeline: String,
}

fn main() {
    match run() {
        Ok(r) => r,
        Err(e) => error!("Error! {}", e),
    }
}

fn run() -> Result<(), Error>  {
    let opts: Opts = Opts::parse();

    let filter = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| DEFAULT_RUST_LOG.to_owned());
    tracing_subscriber::fmt()
        .with_env_filter(filter.clone())
        .with_span_events(FmtSpan::CLOSE)
        .init();

    match std::env::var("GST_DEBUG") {
        Ok(_) => (),
        Err(_) => std::env::set_var("GST_DEBUG", DEFAULT_GST_DEBUG),
    };

    info!("main: BEGIN");
    info!("RUST_LOG={}", filter);
    info!("GST_DEBUG={}", std::env::var("GST_DEBUG").unwrap_or_default());
    info!("opts={:?}", opts);

    gst::init()?;

    let main_loop = glib::MainLoop::new(None, false);
    let server = gst_rtsp_server::RTSPServer::new();
    let mounts = server.mount_points().ok_or(NoMountPoints)?;
    let factory = media_factory::Factory::default();
    let factory: gst_rtsp_server::RTSPMediaFactory = factory.dynamic_cast::<gst_rtsp_server::RTSPMediaFactory>().unwrap();
    
    let share_pipeline = opts.share_pipeline != 0;
    info!("share_pipeline={}", share_pipeline);
    factory.set_shared(share_pipeline);

    if let Some(password) = opts.password {
        info!("Authentication enabled.");
        debug!("User name={}, Password={}", opts.user_name, password);
        let auth = RTSPAuth::new();
        let token = RTSPToken::new(&[(*gst_rtsp_server::RTSP_TOKEN_MEDIA_FACTORY_ROLE, &"user")]);
        let basic = RTSPAuth::make_basic(&opts.user_name[..], &password[..]);
        // This declares that the user "user" (once authenticated) has a role that
        // allows them to access and construct media factories.
        unsafe {
            gst_rtsp_server::ffi::gst_rtsp_media_factory_add_role(
                factory.to_glib_none().0,
                "user".to_glib_none().0,
                gst_rtsp_server::RTSP_PERM_MEDIA_FACTORY_ACCESS
                    .to_glib_none()
                    .0,
                <bool as StaticType>::static_type().into_glib() as *const u8,
                true.into_glib() as *const u8,
                gst_rtsp_server::RTSP_PERM_MEDIA_FACTORY_CONSTRUCT.as_ptr() as *const u8,
                <bool as StaticType>::static_type().into_glib() as *const u8,
                true.into_glib() as *const u8,
                ptr::null_mut::<u8>(),
            );
        }

        if let (Some(cert_file), Some(key_file)) = (opts.tls_cert_file, opts.tls_key_file) {
            let cert_path = Path::new(&cert_file);
            let key_path = Path::new(&key_file);
            let server_cert = TlsCertificate::from_files(cert_path, key_path)?;
            auth.set_tls_certificate(Some(&server_cert));
        }
        
        auth.add_basic(basic.as_str(), &token);
        server.set_auth(Some(&auth));
    }

    server.set_service(&opts.port.to_string()[..]);
    mounts.add_factory(&opts.path[..], &factory);
    let source_id = server.attach(None)?;

    info!(
        "RTSP Camera Simulator ready at rtsp://{}:{}{}",
        server.address().unwrap(),
        server.bound_port(),
        opts.path,
    );

    main_loop.run();

    glib::source_remove(source_id);

    info!("main: END");
    Ok(())
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
            type ParentType = gst_rtsp_server::RTSPMediaFactory;

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
                url: &gst_rtsp::RTSPUrl,
            ) -> Option<gst::Element> {

                let opts: Opts = Opts::parse();

                // Parse the URL to get parameters used to build the pipeline.
                let url = url.request_uri().unwrap().to_string();
                let url = Url::parse(&url[..]).unwrap();
                info!("create_element: Received request: url={:?}", url);
                let query_map: HashMap<_, _> = url.query_pairs().into_owned().collect();
                debug!("create_element: query_map={:?}", query_map);
                let width = match query_map.get("width") {
                    Some(width) => width.clone().parse::<u32>().unwrap_or(opts.width),
                    None => opts.width,
                };
                let height = match query_map.get("height") {
                    Some(height) => height.clone().parse::<u32>().unwrap_or(opts.height),
                    None => opts.height,
                };
                let fps = match query_map.get("fps") {
                    Some(fps) => fps.clone().parse::<f64>().unwrap_or(opts.fps),
                    None => opts.fps,
                };
                let fps = fps as u64;
                let key_frame_interval_max = match query_map.get("key_frame_interval_max") {
                    Some(key_frame_interval_max) => key_frame_interval_max.clone().parse::<u32>().unwrap_or(opts.key_frame_interval_max),
                    None => opts.key_frame_interval_max,
                };
                let target_rate_kilobytes_per_sec = match query_map.get("target_rate_kilobytes_per_sec") {
                    Some(target_rate_kilobytes_per_sec) => target_rate_kilobytes_per_sec.clone().parse::<f64>().unwrap_or(opts.target_rate_kilobytes_per_sec),
                    None => opts.target_rate_kilobytes_per_sec,
                };
                let target_rate_kilobits_per_sec = (target_rate_kilobytes_per_sec * 8.0) as u64;
                let target_rate_bits_per_sec = (target_rate_kilobytes_per_sec * 8000.0) as u64;
                let default_show_clock = opts.show_clock != 0;
                let show_clock = match query_map.get("show_clock") {
                    Some(show_clock) => show_clock.clone().parse::<bool>().unwrap_or(default_show_clock),
                    None => default_show_clock,
                };

                let video_encoder_pipeline = match opts.video_encoder_pipeline.as_str() {
                    "x264enc" => format!("\
                        x264enc \
                        bitrate={target_rate_kbits_per_sec} \
                        key-int-max={key_frame_interval_max} \
                        speed-preset=ultrafast \
                        tune=zerolatency \
                        ",
                        target_rate_kbits_per_sec = target_rate_kilobits_per_sec,
                        key_frame_interval_max = key_frame_interval_max,
                        ),
                    "nvv4l2h264enc" => format!("\
                        nvvideoconvert \
                        ! nvv4l2h264enc \
                        bitrate={target_rate_bits_per_sec} \
                        control-rate=1 \
                        iframeinterval={key_frame_interval_max} \
                        ",
                        target_rate_bits_per_sec = target_rate_bits_per_sec,
                        key_frame_interval_max = key_frame_interval_max,
                    ),
                    p => p.to_owned(),
                };
            
                let clock_overlay_pipeline = if show_clock {
                    format!(" \
                        ! clockoverlay font-desc=\"Sans, 48\" time-format=\"%F %T\" shaded-background=true \
                        ! timeoverlay valignment=bottom font-desc=\"Sans, 48\" shaded-background=true \
                        ") 
                } else {
                    format!("")
                };

                let pipeline_description = format!(
                    "videotestsrc name=src is-live=true do-timestamp=true \
                    ! video/x-raw,width={width},height={height},framerate={fps}/1 \
                    ! videoconvert \
                    {clock_overlay_pipeline} \
                    ! {video_encoder_pipeline} \
                    ! h264parse \
                    ! rtph264pay name=pay0 pt=96",
                    width = width,
                    height = height,
                    fps = fps,
                    clock_overlay_pipeline = clock_overlay_pipeline,
                    video_encoder_pipeline = video_encoder_pipeline,
                );
                info!("create_element: Launch Pipeline: {}", pipeline_description);
                let bin = gst::parse_launch(&pipeline_description.to_owned()).unwrap();
                Some(bin.upcast())
            }
        }
    }

    glib::wrapper! {
        pub struct Factory(ObjectSubclass<imp::Factory>) @extends gst_rtsp_server::RTSPMediaFactory;
    }

    unsafe impl Send for Factory {}
    unsafe impl Sync for Factory {}

    impl Default for Factory {
        fn default() -> Factory {
            glib::Object::new(&[]).expect("Failed to create factory")
        }
    }
}
