// Simulate an RTSP camera.
// Based on gstreamer-rs/examples/src/bin/rtsp-server.rs and rtsp-server-record.rs.

use anyhow::Error;
use clap::Clap;
use derive_more::{Display, Error};
use glib::subclass::prelude::*;
use glib::translate::*;
use gst::prelude::*;
use gst_rtsp_server::prelude::*;
use gst_rtsp_server::{RTSPAuth, RTSPToken};
use gst_rtsp_server::subclass::prelude::*;
use std::ptr;
use std::collections::HashMap;
use tracing_subscriber::fmt::format::FmtSpan;
use url::Url;

#[derive(Debug, Display, Error)]
#[display(fmt = "Could not get mount points")]
struct NoMountPoints;

/// RTSP camera simulator
#[derive(Clap)]
struct Opts {
    /// TCP port to listen on
    #[clap(long, env = "RTSP_PORT", default_value = "8554")]
    port: u16,
    /// URL path to accept
    #[clap(long, env = "RTSP_PATH", default_value = "/cam/realmonitor")]
    path: String,
    /// Default width
    #[clap(long, env = "RTSP_WIDTH", default_value = "640")]
    width: u32,
    /// Default height
    #[clap(long, env = "RTSP_HEIGHT", default_value = "480")]
    height: u32,
    /// User name for basic authentication
    #[clap(long, env = "RTSP_USER_NAME", default_value = "user")]
    user_name: String,
    /// Password for basic authentication. Authentication will be disabled if not specified.
    #[clap(long, env = "RTSP_PASSWORD")]
    password: Option<String>,
}

fn main() {
    match run() {
        Ok(r) => r,
        Err(e) => tracing::error!("Error! {}", e),
    }
}

fn run() -> Result<(), Error>  {
    let opts: Opts = Opts::parse();

    let filter = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| "info".to_owned());
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_span_events(FmtSpan::CLOSE)
        .init();

    gst::init()?;

    let main_loop = glib::MainLoop::new(None, false);
    let server = gst_rtsp_server::RTSPServer::new();
    let mounts = server.get_mount_points().ok_or(NoMountPoints)?;
    let factory = media_factory::Factory::default();
    let factory: gst_rtsp_server::RTSPMediaFactory = factory.dynamic_cast::<gst_rtsp_server::RTSPMediaFactory>().unwrap();

    if let Some(password) = opts.password {
        tracing::info!("Authentication enabled.");
        tracing::debug!("User name={}, Password={}", opts.user_name, password);
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
                <bool as StaticType>::static_type().to_glib() as *const u8,
                true.to_glib() as *const u8,
                gst_rtsp_server::RTSP_PERM_MEDIA_FACTORY_CONSTRUCT.as_ptr() as *const u8,
                <bool as StaticType>::static_type().to_glib() as *const u8,
                true.to_glib() as *const u8,
                ptr::null_mut::<u8>(),
            );
        }
        auth.add_basic(basic.as_str(), &token);
        server.set_auth(Some(&auth));
    }

    server.set_service(&opts.port.to_string()[..]);
    mounts.add_factory(&opts.path[..], &factory);
    let source_id = server.attach(None)?;

    tracing::info!(
        "RTSP Camera Simulator ready at rtsp://{}:{}{}",
        server.get_address().unwrap(),
        server.get_bound_port(),
        opts.path,
    );

    main_loop.run();

    glib::source_remove(source_id);

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
                let url = url.get_request_uri().unwrap().to_string();
                let url = Url::parse(&url[..]).unwrap();
                tracing::info!("url={:?}", url);
                let query_map: HashMap<_, _> = url.query_pairs().into_owned().collect();
                tracing::info!("query_map={:?}", query_map);
                let width = match query_map.get("width") {
                    Some(width) => width.clone().parse::<u32>().unwrap_or(opts.width),
                    None => opts.width,
                };
                let height = match query_map.get("height") {
                    Some(height) => height.clone().parse::<u32>().unwrap_or(opts.height),
                    None => opts.height,
                };

                let pipeline_description =
                        "videotestsrc name=src is-live=true do-timestamp=true".to_owned()
                        + " ! " + &format!("video/x-raw,width={},height={},framerate=30/1", width, height)[..]
                        + " ! videoconvert"
                        + " ! clockoverlay font-desc=\"Sans, 48\" time-format=\"%F %T\" shaded-background=true"
                        + " ! timeoverlay valignment=bottom font-desc=\"Sans, 48\" shaded-background=true"
                        + " ! x264enc key-int-max=30 bitrate=1000"
                        + " ! h264parse"
                        + " ! rtph264pay name=pay0 pt=96"
                    ;
                tracing::info!("Launch Pipeline: {}", pipeline_description);
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
