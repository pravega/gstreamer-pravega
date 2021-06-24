//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

use glib::subclass::prelude::*;
// use gst::ClockTime;
use gst::prelude::*;
use gst::subclass::prelude::*;
#[allow(unused_imports)]
use gst::{gst_debug, gst_error, gst_warning, gst_info, gst_log, gst_trace};
use once_cell::sync::Lazy;
use pravega_client::client_factory::ClientFactory;
use pravega_client::sync::table::{Table, TableError, Version};
use pravega_client_shared::Scope;
use pravega_video::timestamp::PravegaTimestamp;
use pravega_video::utils;
use serde::{Deserialize, Serialize};
// use std::convert::TryInto;
use std::sync::{Arc, Mutex};

pub const ELEMENT_NAME: &str = "pravegatc";
const ELEMENT_CLASS_NAME: &str = "PravegaTC";
const ELEMENT_LONG_NAME: &str = "Pravega Transaction Coordinator";
const ELEMENT_DESCRIPTION: &str = "\
Pravega Transaction Coordinator";
const ELEMENT_AUTHOR: &str = "Claudio Fahey <claudio.fahey@dell.com>";
const DEBUG_CATEGORY: &str = ELEMENT_NAME;

const PROPERTY_NAME_TABLE: &str = "table";
const PROPERTY_NAME_CONTROLLER: &str = "controller";
const PROPERTY_NAME_KEYCLOAK_FILE: &str = "keycloak-file";

const DEFAULT_CONTROLLER: &str = "127.0.0.1:9090";

const PERSISTENT_STATE_TABLE_KEY: &str = "pravegatc.PersistentState";

#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq)]
struct PersistentState {
    pts: u64,
}

#[derive(Debug)]
struct Settings {
    scope: Option<String>,
    table: Option<String>,
    controller: Option<String>,
    keycloak_file: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            scope: None,
            table: None,
            controller: Some(DEFAULT_CONTROLLER.to_owned()),
            keycloak_file: None,
        }
    }
}

// #[derive(Debug)]
struct StartedState {
    table: Arc<Mutex<Table>>,
}

enum State {
    Stopped,
    Started {
        state: StartedState,
    }
}

impl Default for State {
    fn default() -> State {
        State::Stopped
    }
}

pub struct PravegaTC {
    settings: Mutex<Settings>,
    state: Mutex<State>,
    srcpad: gst::Pad,
    sinkpad: gst::Pad,
}

static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new(
        DEBUG_CATEGORY,
        gst::DebugColorFlags::empty(),
        Some(ELEMENT_LONG_NAME),
    )
});

impl PravegaTC {
    fn set_table(
        &self,
        element: &super::PravegaTC,
        table: Option<String>,
    ) -> Result<(), glib::Error> {
        let mut settings = self.settings.lock().unwrap();
        let (scope, table) = match table {
            Some(table) => {
                let components: Vec<&str> = table.split('/').collect();
                if components.len() != 2 {
                    return Err(glib::Error::new(
                        gst::URIError::BadUri,
                        format!("table parameter '{}' is formatted incorrectly. It must be specified as scope/table.", table).as_str(),
                    ));
                }
                let scope = components[0].to_owned();
                let table = components[1].to_owned();
                (Some(scope), Some(table))
            }
            None => {
                gst_info!(CAT, obj: element, "Resetting `{}` to None", PROPERTY_NAME_TABLE);
                (None, None)
            }
        };
        settings.scope = scope;
        settings.table = table;
        Ok(())
    }

    fn set_controller(
        &self,
        _element: &super::PravegaTC,
        controller: Option<String>,
    ) -> Result<(), glib::Error> {
        let mut settings = self.settings.lock().unwrap();
        settings.controller = controller;
        Ok(())
    }

    fn start(&self, element: &super::PravegaTC) -> Result<(), gst::ErrorMessage> {
        gst_debug!(CAT, obj: element, "start: BEGIN");
        let result = (|| {
            let mut state = self.state.lock().unwrap();
            if let State::Started { .. } = *state {
                unreachable!("already started");
            }
            let settings = self.settings.lock().unwrap();
            let scope_name: String = settings.scope.clone().ok_or_else(|| {
                gst::error_msg!(gst::ResourceError::Settings, ["Scope is not defined"])
            })?;
            let table_name = settings.table.clone().ok_or_else(|| {
                gst::error_msg!(gst::ResourceError::Settings, ["Table is not defined"])
            })?;
            let scope = Scope::from(scope_name);
            gst_info!(CAT, obj: element, "start: scope={}, table_name={}", scope, table_name);
            let controller = settings.controller.clone().ok_or_else(|| {
                gst::error_msg!(gst::ResourceError::Settings, ["Controller is not defined"])
            })?;
            gst_info!(CAT, obj: element, "start: controller={}", controller);
            let keycloak_file = settings.keycloak_file.clone();
            gst_info!(CAT, obj: element, "start: keycloak_file={:?}", keycloak_file);
            let config = utils::create_client_config(controller, keycloak_file).map_err(|error| {
                gst::error_msg!(gst::ResourceError::Settings, ["Failed to create Pravega client config: {}", error])
            })?;
            gst_debug!(CAT, obj: element, "start: config={:?}", config);
            gst_info!(CAT, obj: element, "start: controller_uri={}:{}", config.controller_uri.domain_name(), config.controller_uri.port());
            gst_info!(CAT, obj: element, "start: is_tls_enabled={}", config.is_tls_enabled);
            gst_info!(CAT, obj: element, "start: is_auth_enabled={}", config.is_auth_enabled);

            let client_factory = ClientFactory::new(config);
            let runtime = client_factory.runtime();

            // Create Pravega table.
            let table = runtime.block_on(client_factory.create_table(scope, table_name));

            let test_pts = 981172837000000000 * gst::NSECOND + 5 * gst::SECOND - 1 * gst::MSECOND;
            let v = PersistentState {
                pts: test_pts.nanoseconds().unwrap(),
            };
            runtime.block_on(table.insert(&PERSISTENT_STATE_TABLE_KEY.to_string(), &v, -1)).unwrap();

            // Get last checkpointed state (pts) from Pravega table.
            let persistent_state: Result<Option<(PersistentState, Version)>, TableError> = runtime.block_on(table.get(&PERSISTENT_STATE_TABLE_KEY.to_string()));
            gst_info!(CAT, obj: element, "start: persistent_state={:?}", persistent_state);
            let persistent_state = persistent_state.unwrap();
            match persistent_state {
                Some((persistent_state, _)) => {
                    let timestamp = PravegaTimestamp::from_nanoseconds(Some(persistent_state.pts));
                    gst_log!(CAT, obj: element, "start: persistent state timestamp={:?}", timestamp);
                    let pipeline = element.parent().unwrap().downcast::<gst::Pipeline>().unwrap();
                    // gst_log!(CAT, obj: element, "start: parent={:?}", pipeline);
                    // gst_log!(CAT, obj: element, "start: parent.name={:?}", pipeline.name());
                    // let children = pipeline.children();
                    // gst_log!(CAT, obj: element, "start: children={:?}", children);
                    // TODO: Find all pravegasrc elements and set start-timestamp property.
                    let src = pipeline.child_by_name("src").unwrap();
                    gst_log!(CAT, obj: element, "start: src={:?}", src);
                    src.set_property_from_str("start-mode", "timestamp");
                    src.set_property("start-timestamp", &timestamp.nanoseconds().unwrap()).unwrap();
                },
                None => {
                },
            }

            *state = State::Started {
                state: StartedState {
                    table: Arc::new(Mutex::new(table)),
                },
            };
            gst_info!(CAT, obj: element, "start: Started");
            Ok(())
        })();
        gst_debug!(CAT, obj: element, "start: END: result={:?}", result);
        result
    }

    fn sink_chain(
        &self,
        pad: &gst::Pad,
        element: &super::PravegaTC,
        buffer: gst::Buffer,
    ) -> Result<gst::FlowSuccess, gst::FlowError> {
        gst_log!(CAT, obj: pad, "sink_chain: Handling buffer {:?}", buffer);

        let mut state = self.state.lock().unwrap();

        let state = match *state {
            State::Started {
                ref mut state,
                ..
            } => state,
            State::Stopped => {
                panic!("Not started yet");
            }
    };

        self.srcpad.push(buffer)
        // gst_trace!(CAT, obj: element, "sink_chain: END: state={:?}", state);
        // Ok(gst::FlowSuccess::Ok)
    }

    fn sink_event(&self, _pad: &gst::Pad, _element: &super::PravegaTC, event: gst::Event) -> bool {
        self.srcpad.push_event(event)
    }

    fn sink_query(&self, _pad: &gst::Pad, _element: &super::PravegaTC, query: &mut gst::QueryRef) -> bool {
        self.srcpad.peer_query(query)
    }

    fn src_event(&self, _pad: &gst::Pad, _element: &super::PravegaTC, event: gst::Event) -> bool {
        self.sinkpad.push_event(event)
    }

    fn src_query(&self, _pad: &gst::Pad, _element: &super::PravegaTC, query: &mut gst::QueryRef) -> bool {
        self.sinkpad.peer_query(query)
    }

    fn stop(&self, element: &super::PravegaTC) -> Result<(), gst::ErrorMessage> {
        gst_info!(CAT, obj: element, "stop: BEGIN");
        let result = (|| {
            let mut state = self.state.lock().unwrap();
            if let State::Stopped = *state {
                return Err(gst::error_msg!(
                    gst::ResourceError::Settings,
                    ["not started"]
                ));
            }
            *state = State::Stopped;
            Ok(())
        })();
        gst_info!(CAT, obj: element, "stop: END: result={:?}", result);
        result
    }
}

#[glib::object_subclass]
impl ObjectSubclass for PravegaTC {
    const NAME: &'static str = ELEMENT_CLASS_NAME;
    type Type = super::PravegaTC;
    type ParentType = gst::Element;

    fn with_class(klass: &Self::Class) -> Self {
        pravega_video::tracing::init();

        let templ = klass.pad_template("sink").unwrap();
        let sinkpad = gst::Pad::builder_with_template(&templ, Some("sink"))
            .chain_function(|pad, parent, buffer| {
                PravegaTC::catch_panic_pad_function(
                    parent,
                    || Err(gst::FlowError::Error),
                    |identity, element| identity.sink_chain(pad, element, buffer),
                )
            })
            .event_function(|pad, parent, event| {
                PravegaTC::catch_panic_pad_function(
                    parent,
                    || false,
                    |identity, element| identity.sink_event(pad, element, event),
                )
            })
            .query_function(|pad, parent, query| {
                PravegaTC::catch_panic_pad_function(
                    parent,
                    || false,
                    |identity, element| identity.sink_query(pad, element, query),
                )
            })
            .build();

        let templ = klass.pad_template("src").unwrap();
        let srcpad = gst::Pad::builder_with_template(&templ, Some("src"))
            .event_function(|pad, parent, event| {
                PravegaTC::catch_panic_pad_function(
                    parent,
                    || false,
                    |identity, element| identity.src_event(pad, element, event),
                )
            })
            .query_function(|pad, parent, query| {
                PravegaTC::catch_panic_pad_function(
                    parent,
                    || false,
                    |identity, element| identity.src_query(pad, element, query),
                )
            })
            .build();

        Self {
            settings: Mutex::new(Default::default()),
            state: Mutex::new(Default::default()),
            srcpad,
            sinkpad,
        }
    }
}

impl ObjectImpl for PravegaTC {
    fn constructed(&self, obj: &Self::Type) {
        self.parent_constructed(obj);
        obj.add_pad(&self.sinkpad).unwrap();
        obj.add_pad(&self.srcpad).unwrap();
    }

    fn properties() -> &'static [glib::ParamSpec] {
        static PROPERTIES: Lazy<Vec<glib::ParamSpec>> = Lazy::new(|| { vec![
            glib::ParamSpec::new_string(
                PROPERTY_NAME_TABLE,
                "Table",
                "scope/table",
                None,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpec::new_string(
                PROPERTY_NAME_CONTROLLER,
                "Controller",
                "Pravega controller",
                None,
                glib::ParamFlags::WRITABLE,
            ),
            glib::ParamSpec::new_string(
                PROPERTY_NAME_KEYCLOAK_FILE,
                "Keycloak file",
                "The filename containing the Keycloak credentials JSON. If missing or empty, authentication will be disabled.",
                None,
                glib::ParamFlags::WRITABLE,
            ),
        ]});
        PROPERTIES.as_ref()
    }

    // TODO: On error, should set flag that will cause element to fail.
    fn set_property(
        &self,
        obj: &Self::Type,
        _id: usize,
        value: &glib::Value,
        pspec: &glib::ParamSpec,
    ) {
        match pspec.name() {
            PROPERTY_NAME_TABLE => {
                let res = match value.get::<String>() {
                    Ok(table) => self.set_table(&obj, Some(table)),
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_TABLE, err);
                }
            },
            PROPERTY_NAME_CONTROLLER => {
                let res = match value.get::<String>() {
                    Ok(controller) => {
                        let controller = if controller.is_empty() {
                            None
                        } else {
                            Some(controller)
                        };
                        self.set_controller(&obj, controller)
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_CONTROLLER, err);
                }
            },
            PROPERTY_NAME_KEYCLOAK_FILE => {
                let res: Result<(), glib::Error> = match value.get::<String>() {
                    Ok(keycloak_file) => {
                        let mut settings = self.settings.lock().unwrap();
                        settings.keycloak_file = if keycloak_file.is_empty() {
                            None
                        } else {
                            Some(keycloak_file)
                        };
                        Ok(())
                    },
                    Err(_) => unreachable!("type checked upstream"),
                };
                if let Err(err) = res {
                    gst_error!(CAT, obj: obj, "Failed to set property `{}`: {}", PROPERTY_NAME_KEYCLOAK_FILE, err);
                }
            },
        _ => unimplemented!(),
        };
    }
}

impl ElementImpl for PravegaTC {
    fn metadata() -> Option<&'static gst::subclass::ElementMetadata> {
        static ELEMENT_METADATA: Lazy<gst::subclass::ElementMetadata> = Lazy::new(|| {
            gst::subclass::ElementMetadata::new(
                ELEMENT_LONG_NAME,
                "Generic",
                ELEMENT_DESCRIPTION,
                ELEMENT_AUTHOR,
                )
        });
        Some(&*ELEMENT_METADATA)
    }

    fn pad_templates() -> &'static [gst::PadTemplate] {
        static PAD_TEMPLATES: Lazy<Vec<gst::PadTemplate>> = Lazy::new(|| {
            let caps = gst::Caps::new_any();
            let src_pad_template = gst::PadTemplate::new(
                "src",
                gst::PadDirection::Src,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();
            let sink_pad_template = gst::PadTemplate::new(
                "sink",
                gst::PadDirection::Sink,
                gst::PadPresence::Always,
                &caps,
            )
            .unwrap();
            vec![src_pad_template, sink_pad_template]
        });
        PAD_TEMPLATES.as_ref()
    }

    fn change_state(
        &self,
        element: &Self::Type,
        transition: gst::StateChange,
    ) -> Result<gst::StateChangeSuccess, gst::StateChangeError> {
        gst_trace!(CAT, obj: element, "change_state: Changing state {:?}", transition);
        match transition {
            gst::StateChange::ReadyToPaused => {
                self.start(element).unwrap();
            },
            gst::StateChange::PausedToReady => {
                self.stop(element).unwrap();
            },
            _ => {}
        }
        self.parent_change_state(element, transition)
    }
}
