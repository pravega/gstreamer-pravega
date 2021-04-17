
use glib::prelude::*;

mod imp;

// The public Rust wrapper type for our element
glib::wrapper! {
    pub struct PravegaSrc(ObjectSubclass<imp::PravegaSrc>) @extends gst_base::BaseSrc, gst::Element, gst::Object;
}

// GStreamer elements need to be thread-safe. For the private implementation this is automatically
// enforced but for the public wrapper type we need to specify this manually.
unsafe impl Send for PravegaSrc {}
unsafe impl Sync for PravegaSrc {}

// Registers the type for our element, and then registers in GStreamer under
// the name "rsrgb2gray" for being able to instantiate it via e.g.
// gst::ElementFactory::make().
pub fn register(plugin: &gst::Plugin) -> Result<(), glib::BoolError> {
    gst::Element::register(
        Some(plugin),
        "pravegasrc",
        gst::Rank::None,
        PravegaSrc::static_type(),
    )
}
