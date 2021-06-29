//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

// An implementation of NVIDIA DeepStream Message Broker for a Pravega event stream.
// See https://docs.nvidia.com/metropolis/deepstream/dev-guide/text/DS_plugin_gst-nvmsgbroker.html

use anyhow::anyhow;
use pravega_client::client_factory::ClientFactory;
use pravega_client::event::EventWriter;
use pravega_client_shared::{StreamConfiguration, ScopedStream, Scaling, ScaleType};
use std::collections::HashMap;
use std::ffi::CStr;
use std::fmt;
use std::os::raw::c_char;
use std::ptr;
use std::sync::{Arc, Once};
use tokio::sync::Mutex;
use tracing::{debug, error, info, trace};
use tracing_subscriber::fmt::format::FmtSpan;
use configparser::ini::Ini;
use pravega_video::utils;

static TRACING_INIT: Once = Once::new();

/// Initialize tracing.
/// If the environment variable PRAVEGA_PROTOCOL_ADAPTER_LOG is unset, we output all info events to stdout.
/// If PRAVEGA_PROTOCOL_ADAPTER_LOG is set to an empty string, this function does not configure any tracing subscribers.
fn init_tracing() {
    TRACING_INIT.call_once(|| {
        let filter = std::env::var("PRAVEGA_PROTOCOL_ADAPTER_LOG")
            .unwrap_or_else(|_| "nvds_pravega_proto=info,warn".to_owned());
        if !filter.is_empty() {
            // This will fail if there is already a global default tracing subscriber.
            // Any such errors will be ignored.
            let _ = tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_span_events(FmtSpan::CLOSE)
                .try_init();
        }
    })
}

/// Convert C string to Rust String.
fn c_string_to_string(s: *const c_char) -> Option<String> {
    if s.is_null() {
        None
    } else {
        let s = unsafe { CStr::from_ptr(s) };
        Some(s.to_string_lossy().into_owned())
    }
}

const NVDS_MSGAPI_VERSION_SZ: &str = "2.0\0";
const NVDS_MSGAPI_PROTOCOL_SZ: &str = "PRAVEGA\0";

#[repr(C)]
#[allow(non_camel_case_types)]
/// From nvds_msgapi.h
pub enum NvDsMsgApiErrorType {
    NVDS_MSGAPI_OK,
    NVDS_MSGAPI_ERR,
    NVDS_MSGAPI_UNKNOWN_TOPIC,
}

#[derive(Clone, Debug)]
pub enum RoutingKeyMethod {
    /// The routing key will be the specified string.
    Fixed { routing_key: String },
}
/// This provides a pool of EventWriter instances with one instance per stream.
/// Instances are created dynamically for any new streams.
/// Instances are not dropped until the pool is dropped.
pub struct EventWriterPool {
    pub client_factory: ClientFactory,
    pub writers: Mutex<HashMap<ScopedStream, Arc<Mutex<EventWriter>>>>,
}

impl EventWriterPool {
    pub fn new(client_factory: ClientFactory) -> Self {
        EventWriterPool {
            client_factory,
            writers: Mutex::new(HashMap::new()),
        }
    }

    pub async fn get_or_create(&self, scoped_stream: ScopedStream) -> Arc<Mutex<EventWriter>> {
        let mut writers = self.writers.lock().await;
        let writer = writers.get(&scoped_stream.clone());
        match writer {
            Some(writer) => {
                debug!("EventWriterPool::get_or_create: Using existing writer for {}", scoped_stream);
                writer.clone()
            },
            None => {
                info!("EventWriterPool::get_or_create: Creating new writer for {}", scoped_stream);
                // Create stream if needed.
                let controller_client = self.client_factory.controller_client();
                // This StreamConfiguration will be used only if the stream does not yet exist.
                // If the stream already exists, it will not be changed.
                let stream_config = StreamConfiguration {
                    scoped_stream: scoped_stream.clone(),
                    scaling: Scaling {
                        scale_type: ScaleType::FixedNumSegments,
                        min_num_segments: 1,
                        ..Default::default()
                    },
                    retention: Default::default(),
                };
                let create_stream_result = controller_client.create_stream(&stream_config).await.unwrap();
                info!("EventWriterPool::get_or_create: Stream created, create_stream_result={}", create_stream_result);
                let writer = self.client_factory.create_event_writer(scoped_stream.clone());
                let writer = Arc::new(Mutex::new(writer));
                writers.insert(scoped_stream.clone(), writer.clone());
                writer
            },
        }
    }
}

impl fmt::Debug for EventWriterPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventWriterPool")
            // .field("writers", &self.writers)
            .finish()
    }
}

pub struct NvDsPravegaClientHandle {
    pub client_factory: ClientFactory,
    pub writer_pool: EventWriterPool,
    pub routing_key_method: RoutingKeyMethod,
}

impl NvDsPravegaClientHandle {
    pub fn new(client_factory: ClientFactory, routing_key_method: RoutingKeyMethod) -> Self {
        NvDsPravegaClientHandle {
            client_factory: client_factory.clone(),
            writer_pool: EventWriterPool::new(client_factory.clone()),
            routing_key_method,
        }
    }

    pub fn resolve_topic(&self, topic: String) -> Result<ScopedStream, std::convert::Infallible> {
        Ok(ScopedStream::from(&topic[..]))
    }
}

impl fmt::Debug for NvDsPravegaClientHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NvDsPravegaClientHandle")
            // .field("writer_pool", &self.writer_pool)
            .finish()
    }
}

// char *nvds_msgapi_getversion()
#[no_mangle]
pub extern "C" fn nvds_msgapi_getversion() -> *const u8 {
    return NVDS_MSGAPI_VERSION_SZ.as_ptr();
}

// char *nvds_msgapi_get_protocol_name()
#[no_mangle]
pub extern "C" fn nvds_msgapi_get_protocol_name() -> *const u8 {
    return NVDS_MSGAPI_PROTOCOL_SZ.as_ptr();
}

// NvDsMsgApiErrorType nvds_msgapi_connection_signature(char *broker_str, char *cfg, char *output_str, int max_len)
#[no_mangle]
pub extern "C" fn nvds_msgapi_connection_signature(_broker_str: *const c_char, _cfg: *const c_char, output_str: *mut u8, _max_len: isize)
        -> NvDsMsgApiErrorType {
    init_tracing();
    // Connection sharing is not implemented so we simply set output_str to an empty null-terminated string.
    unsafe { *output_str = 0; }
    return NvDsMsgApiErrorType::NVDS_MSGAPI_OK;
}

// NvDsMsgApiHandle nvds_msgapi_connect(char *connection_str,  nvds_msgapi_connect_cb_t connect_cb, char *config_path)
#[no_mangle]
pub extern "C" fn nvds_msgapi_connect(
        connection_str: *const c_char, 
        // typedef void (*nvds_msgapi_connect_cb_t)(NvDsMsgApiHandle h_ptr, NvDsMsgApiEventType ds_evt);
        connect_cb: extern "C" fn(h_ptr: *mut NvDsPravegaClientHandle, ds_event: usize),
        config_path: *const c_char)
        -> *const NvDsPravegaClientHandle {
    init_tracing();
    let connection_str = c_string_to_string(connection_str).unwrap_or(String::from("tcp://127.0.0.1:9090"));
    let config_path = c_string_to_string(config_path);
    info!("nvds_msgapi_connect: connection_str={:?}, connect_cb={:?}, config_path={:?}", connection_str, connect_cb, config_path);

    let (routing_key_method, keycloak_file) = match config_path {
        Some(path) => {
            let mut config = Ini::new();
            if let Err(e) = config.load(&path[..]) {
                error!("nvds_msgapi_connect: Failed to load config file: {}", e);
                return ptr::null();
            };
            let routing_key_method = if let Some(fixed_routing_key) = config.get("message-broker", "fixed-routing-key") {
                RoutingKeyMethod::Fixed { routing_key: fixed_routing_key }
            } else {
                RoutingKeyMethod::Fixed { routing_key: "".to_owned() }
            };
            let keycloak_file = config.get("message-broker", "keycloak-file");
            (routing_key_method, keycloak_file)
        },
        None => (RoutingKeyMethod::Fixed { routing_key: "".to_owned() }, None),
    };
    
    info!("nvds_msgapi_connect: controller_uri={:?}, routing_key_method={:?}, keycloak_file={:?}", connection_str, routing_key_method, keycloak_file);

    let client_config = match utils::create_client_config(connection_str, keycloak_file) {
        Ok(config) => config,
        Err(e) => {
            error!("nvds_msgapi_connect: Failed to create Pravega client config: {}", e);
            return ptr::null();
        },
    };
    let client_factory = ClientFactory::new(client_config);
    let client_handle = Box::new(NvDsPravegaClientHandle::new(client_factory, routing_key_method));
    // Prevent Rust from dropping the NvDsPravegaClientHandle instance when this function ends.
    // This will be dropped manually in nvds_msgapi_disconnect().
    let h_ptr = Box::leak(client_handle);
    info!("nvds_msgapi_connect: Pravega client factory created.");
    return h_ptr;
}

// NvDsMsgApiErrorType nvds_msgapi_disconnect(NvDsMsgApiHandle h_ptr)
#[no_mangle]
pub extern "C" fn nvds_msgapi_disconnect(h_ptr: *mut NvDsPravegaClientHandle) -> NvDsMsgApiErrorType {
    let client_handle = unsafe { Box::from_raw(h_ptr) };
    info!("nvds_msgapi_disconnect: client_handle={:?}", client_handle);
    drop(client_handle);
    debug!("nvds_msgapi_disconnect: END");
    return NvDsMsgApiErrorType::NVDS_MSGAPI_OK;
}

// NvDsMsgApiErrorType nvds_msgapi_send(NvDsMsgApiHandle h_ptr, char *topic, const uint8_t *payload, size_t nbuf)
#[no_mangle]
pub extern "C" fn nvds_msgapi_send(h_ptr: *mut NvDsPravegaClientHandle, topic: *const c_char, payload: *const u8, nbuf: usize)
        -> NvDsMsgApiErrorType {
    debug!("nvds_msgapi_send: BEGIN");
    let client_handle: &NvDsPravegaClientHandle = unsafe { &*h_ptr };
    let topic = c_string_to_string(topic).unwrap();
    debug!("nvds_msgapi_send: h_ptr={:?}, client_handle={:?}, topic={:?}, payload={:?}, nbuf={}", 
        h_ptr, client_handle, topic, payload, nbuf);
    let payload = unsafe {std::slice::from_raw_parts(payload, nbuf)};
    let payload_string = String::from_utf8_lossy(payload);
    trace!("nvds_msgapi_send: payload_string={}", payload_string);
    let scoped_stream = client_handle.resolve_topic(topic).unwrap();
    let runtime = client_handle.client_factory.runtime();
    let routing_key_method = client_handle.routing_key_method.clone();
    let routing_key = match routing_key_method {
        RoutingKeyMethod::Fixed { routing_key } => routing_key,
    };
    debug!("nvds_msgapi_send: routing_key={:?}", routing_key);
    let result = runtime.block_on(async {
        // Get a reference to the writer for this topic from the writer pool.
        let writer = client_handle.writer_pool.get_or_create(scoped_stream).await;
        // Get the mutex for this writer so we can use it.
        let mut writer = writer.lock().await;
        let event = payload.to_vec();
        debug!("nvds_msgapi_send: Calling write_event");
        let future = writer.write_event_by_routing_key(routing_key, event);
        let receiver = future.await;
        let result = receiver.await;
        debug!("nvds_msgapi_send: write_event completed; result={:?}", result);
        result
    });
    let result = match result {
        Ok(r) => r.map_err(|e| anyhow!(e)),
        Err(e) => Err(anyhow!(e)),
    };
    let result = match result {
        Ok(_) => {
            // Event has been durably persisted.
            NvDsMsgApiErrorType::NVDS_MSGAPI_OK
        },
    Err(e) => {
            error!("nvds_msgapi_send: write_event error: {:?}", e);
            NvDsMsgApiErrorType::NVDS_MSGAPI_ERR
        },
    };
    debug!("nvds_msgapi_send: END");
    return result;
}

// NvDsMsgApiErrorType nvds_msgapi_send_async(NvDsMsgApiHandle h_ptr, char *topic, const uint8_t *payload, size_t nbuf, nvds_msgapi_send_cb_t send_callback, void *user_ptr)
#[no_mangle]
pub extern "C" fn nvds_msgapi_send_async(
        h_ptr: *mut NvDsPravegaClientHandle, topic: *const c_char, payload: *const u8, nbuf: usize,
        // void test_send_cb(void *user_ptr, NvDsMsgApiErrorType completion_flag)
        cb: extern "C" fn(user_ptr: usize, completion_flag: NvDsMsgApiErrorType),
        user_ptr: usize)
        -> NvDsMsgApiErrorType {
    debug!("nvds_msgapi_send_async: BEGIN");
    let client_handle: &NvDsPravegaClientHandle = unsafe { &*h_ptr };
    let topic = c_string_to_string(topic).unwrap();
    debug!("nvds_msgapi_send_async: h_ptr={:?}, client_handle={:?}, topic={:?}, payload={:?}, nbuf={}, cb={:?}, user_ptr={:?}",
        h_ptr, client_handle, topic, payload, nbuf, cb, user_ptr);
    let payload = unsafe {std::slice::from_raw_parts(payload, nbuf)};
    // Log the payload. This assumes it is a UTF-8 string.
    let payload_string = String::from_utf8_lossy(payload);
    trace!("nvds_msgapi_send_async: payload_string={}", payload_string);
    // Convert unsafe payload to a vector. This also copies the payload which is critical to avoid memory corruption.
    let event = payload.to_vec();
    let scoped_stream = client_handle.resolve_topic(topic).unwrap();
    let runtime = client_handle.client_factory.runtime();
    let routing_key_method = client_handle.routing_key_method.clone();
    let routing_key = match routing_key_method {
        RoutingKeyMethod::Fixed { routing_key } => routing_key,
    };
    debug!("nvds_msgapi_send_async: routing_key={:?}", routing_key);
    // Spawn a task in the Tokio runtime that will write the event, wait for it to be durably persisted,
    // and then call the callback function.
    runtime.spawn(async move {
        // Get a reference to the writer for this topic from the writer pool.
        let writer = client_handle.writer_pool.get_or_create(scoped_stream).await;
        // Get the mutex for this writer so we can use it.
        let mut writer = writer.lock().await;        
        debug!("nvds_msgapi_send_async: Calling write_event_by_routing_key");
        let future = writer.write_event_by_routing_key(routing_key, event);
        let receiver = future.await;
        let result = receiver.await;
        debug!("nvds_msgapi_send_async: write_event_by_routing_key completed; result={:?}", result);
        let result = match result {
            Ok(r) => r.map_err(|e| anyhow!(e)),
            Err(e) => Err(anyhow!(e)),
        };
        let result = match result {
            Ok(_) => {
                // Event has been durably persisted.
                NvDsMsgApiErrorType::NVDS_MSGAPI_OK
            },
            Err(e) => {
                error!("nvds_msgapi_send_async: write_event error: {:?}", e);
                NvDsMsgApiErrorType::NVDS_MSGAPI_ERR
            },
        };
            // Call callback function.
        cb(user_ptr, result);
        });

        debug!("nvds_msgapi_send_async: END");
    return NvDsMsgApiErrorType::NVDS_MSGAPI_OK;
}

// NvDsMsgApiErrorType nvds_msgapi_subscribe(NvDsMsgApiHandle h_ptr, char ** topics, int num_topics, nvds_msgapi_subscribe_request_cb_t cb, void *user_ctx) {
#[no_mangle]
pub extern "C" fn nvds_msgapi_subscribe(
        _h_ptr: *mut NvDsPravegaClientHandle, _topics: *const *const c_char, _num_topics: isize,
        _cb: usize, _user_ctx: usize)
        -> NvDsMsgApiErrorType {
    error!("nvds_msgapi_subscribe: Not implemented");
    // TODO: Implement nvds_msgapi_subscribe.
    // Return OK and behave as though no messages are received.
    return NvDsMsgApiErrorType::NVDS_MSGAPI_OK;
}

// void nvds_msgapi_do_work(NvDsMsgApiHandle h_ptr)
#[no_mangle]
pub extern "C" fn nvds_msgapi_do_work(_h_ptr: *mut NvDsPravegaClientHandle) {
}
