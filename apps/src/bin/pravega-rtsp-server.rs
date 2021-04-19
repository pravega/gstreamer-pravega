// Based on gstreamer-rs/examples/src/bin/rtsp-server.rs.
// This example demonstrates how to set up a rtsp server using GStreamer.
// For this, the example parses an arbitrary pipeline in launch syntax
// from the cli and provides this pipeline's output as stream, served
// using GStreamers rtsp server.

use anyhow::Error;
use clap::Clap;
use derive_more::{Display, Error};
use glib::subclass::prelude::*;
use gst::prelude::*;
use gst_rtsp_server::prelude::*;
use gst_rtsp_server::subclass::prelude::*;
use log::info;
use std::collections::HashMap;
use url::Url;

#[derive(Debug, Display, Error)]
#[display(fmt = "Could not get mount points")]
struct NoMountPoints;

/// Pravega RTSP server
#[derive(Clap)]
struct Opts {
    /// Pravega controller in format "127.0.0.1:9090"
    #[clap(short, long, default_value = "127.0.0.1:9090")]
    controller: String,
    /// Pravega scope
    #[clap(short, long)]
    scope: String,
}

fn main() {
    match run() {
        Ok(r) => r,
        Err(e) => eprintln!("Error! {}", e),
    }
}

fn run() -> Result<(), Error>  {
    env_logger::init();
    let _opts: Opts = Opts::parse();

    // Initialize GStreamer
    gst::init()?;

    let main_loop = glib::MainLoop::new(None, false);
    let server = gst_rtsp_server::RTSPServer::new();
    // Much like HTTP servers, RTSP servers have multiple endpoints that
    // provide different streams. Here, we ask our server to give
    // us a reference to his list of endpoints, so we can add our
    // test endpoint, providing the pipeline from the cli.
    let mounts = server.mount_points().ok_or(NoMountPoints)?;

    // // Next, we create a factory for the endpoint we want to create.
    // // The job of the factory is to create a new pipeline for each client that
    // // connects, or (if configured to do so) to reuse an existing pipeline.
    // let factory = gst_rtsp_server::RTSPMediaFactory::new();

    // Next, we create our custom factory for the endpoint we want to create.
    // The job of the factory is to create a new pipeline for each client that
    // connects, or (if configured to do so) to reuse an existing pipeline.
    let factory = media_factory::Factory::default();

    // Here we tell the media factory the media we want to serve.
    // This is done in the launch syntax. When the first client connects,
    // the factory will use this syntax to create a new pipeline instance.
    // factory.set_launch(args[1].as_str());

    // This setting specifies whether each connecting client gets the output
    // of a new instance of the pipeline, or whether all connected clients share
    // the output of the same pipeline.
    // If you want to stream a fixed video you have stored on the server to any
    // client, you would not set this to shared here (since every client wants
    // to start at the beginning of the video). But if you want to distribute
    // a live source, you will probably want to set this to shared, to save
    // computing and memory capacity on the server.
    // factory.set_shared(true);

    // Now we add a new mount-point and tell the RTSP server to serve the content
    // provided by the factory we configured above, when a client connects to
    // this specific path.
    mounts.add_factory("/test", &factory);

    // Attach the server to our main context.
    // A main context is the thing where other stuff is registering itself for its
    // events (e.g. sockets, GStreamer bus, ...) and the main loop is something that
    // polls the main context for its events and dispatches them to whoever is
    // interested in them. In this example, we only do have one, so we can
    // leave the context parameter empty, it will automatically select
    // the default one.
    let id = server.attach(None)?;

    println!(
        "Stream ready at rtsp://127.0.0.1:{}/test",
        server.bound_port()
    );

    // Start the mainloop. From this point on, the server will start to serve
    // our quality content to connecting clients.
    main_loop.run();

    glib::source_remove(id);

    Ok(())
}

// Our custom media factory that creates a media input manually
mod media_factory {
    use super::*;
    use glib::subclass;

    // In the imp submodule we include the actual implementation
    mod imp {
        use super::*;

        // This is the private data of our factory
        pub struct Factory {}

        // This trait registers our type with the GObject object system and
        // provides the entry points for creating a new instance and setting
        // up the class data
        #[glib::object_subclass]
        impl ObjectSubclass for Factory {
            const NAME: &'static str = "RsRTSPMediaFactory";
            type Type = super::Factory;
            type ParentType = gst_rtsp_server::RTSPMediaFactory;

            // Called when a new instance is to be created. We need to return an instance
            // of our struct here.
            fn new() -> Self {
                Self {}
            }
        }

        // Implementation of glib::Object virtual methods
        impl ObjectImpl for Factory {
            fn constructed(&self, factory: &Self::Type) {
                self.parent_constructed(factory);
                // All media created by this factory are our custom media type. This would
                // not require a media factory subclass and can also be called on the normal
                // RTSPMediaFactory.
                factory.set_media_gtype(super::media::Media::static_type());
            }
        }

        // Implementation of gst_rtsp_server::RTSPMediaFactory virtual methods
        impl RTSPMediaFactoryImpl for Factory {
            fn create_element(
                &self,
                _factory: &Self::Type,
                url: &gst_rtsp::RTSPUrl,
            ) -> Option<gst::Element> {
                let url = url.request_uri().unwrap().to_string();
                let url = Url::parse(&url[..]).unwrap();
                info!("url={:?}", url);
                let query_map: HashMap<_, _> = url.query_pairs().into_owned().collect();
                info!("query_map={:?}", query_map);
                let stream = query_map.get("stream").unwrap().clone();
                info!("stream={:?}", stream);
                let opts: Opts = Opts::parse();
                let pipeline_description = format!(
                    "pravegasrc stream={}/{} controller={} ! tsdemux ! h264parse ! rtph264pay name=pay0 pt=96",
                    opts.scope, stream, opts.controller);
                info!("Launch Pipeline: {}", pipeline_description);
                let bin = gst::parse_launch(&pipeline_description.to_owned()).unwrap();
                Some(bin.upcast())

                // let bin = gst::Bin::new(None);
                // let pravegasrc = gst::ElementFactory::make("pravegasrc", None).unwrap();
                // pravegasrc.set_property("controller", &"192.168.1.123:9090".to_value()).unwrap();
                // pravegasrc.set_property("stream", &"examples/demo18".to_value()).unwrap();
                // let demux = gst::ElementFactory::make("tsdemux", None).unwrap();
                // let pay = gst::ElementFactory::make("rtph264pay", Some("pay0")).unwrap();
                // pay.set_property("pt", &96u32.to_value()).unwrap();
                // bin.add_many(&[&pravegasrc, &demux, &pay]).unwrap();
                // gst::Element::link_many(&[&pravegasrc, &demux, &pay]).unwrap();
                // Some(bin.upcast())
            }
        }
    }

    // This here defines the public interface of our factory and implements
    // the corresponding traits so that it behaves like any other RTSPMediaFactory
    glib::wrapper! {
        pub struct Factory(ObjectSubclass<imp::Factory>) @extends gst_rtsp_server::RTSPMediaFactory;
    }

    // Factories must be Send+Sync, and ours is
    unsafe impl Send for Factory {}
    unsafe impl Sync for Factory {}

    impl Default for Factory {
        // Creates a new instance of our factory
        fn default() -> Factory {
            glib::Object::new(&[]).expect("Failed to create factory")
        }
    }
}

// Our custom media subclass that adds a custom attribute to the SDP returned by DESCRIBE
mod media {
    use super::*;
    use glib::subclass;
    use glib::subclass::prelude::*;
    use gst_rtsp_server::subclass::prelude::*;

    // In the imp submodule we include the actual implementation
    mod imp {
        use super::*;

        // This is the private data of our media
        pub struct Media {}

        // This trait registers our type with the GObject object system and
        // provides the entry points for creating a new instance and setting
        // up the class data
        #[glib::object_subclass]
        impl ObjectSubclass for Media {
            const NAME: &'static str = "RsRTSPMedia";
            type Type = super::Media;
            type ParentType = gst_rtsp_server::RTSPMedia;

            // Called when a new instance is to be created. We need to return an instance
            // of our struct here.
            fn new() -> Self {
                info!("Created custom media");
                Self {}
            }
        }

        // Implementation of glib::Object virtual methods
        impl ObjectImpl for Media {}

        // Implementation of gst_rtsp_server::RTSPMedia virtual methods
        impl RTSPMediaImpl for Media {
            fn setup_sdp(
                &self,
                media: &Self::Type,
                sdp: &mut gst_sdp::SDPMessageRef,
                info: &gst_rtsp_server::subclass::SDPInfo,
            ) -> Result<(), gst::LoggableError> {
                self.parent_setup_sdp(media, sdp, info)?;

                sdp.add_attribute("my-custom-attribute", Some("has-a-value"));

                Ok(())
            }

            fn query_stop(&self, media: &Self::Type) -> Option<gst::ClockTime> {
                info!("query_stop: BEGIN");
                let result = self.parent_query_stop(media);
                info!("query_stop: END; result={:?}", result);
                result
            }
        }
    }

    // This here defines the public interface of our factory and implements
    // the corresponding traits so that it behaves like any other RTSPMedia
    glib::wrapper! {
        pub struct Media(ObjectSubclass<imp::Media>) @extends gst_rtsp_server::RTSPMedia;
    }

    // Medias must be Send+Sync, and ours is
    unsafe impl Send for Media {}
    unsafe impl Sync for Media {}
}
