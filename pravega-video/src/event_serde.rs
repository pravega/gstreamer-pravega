//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

// Module for serialization of events for writing to a Pravega byte stream.

use std::convert::TryInto;
use std::io::{Error, ErrorKind, Read, Write};
use enumflags2::BitFlags;
use crate::timestamp::PravegaTimestamp;

#[derive(BitFlags, Copy, Clone, Debug, PartialEq)]
#[repr(u8)]
pub(crate) enum EventHeaderFlags {
    IncludeInIndex         = 0b00000001,
    RandomAccessIndicator  = 0b00000010,
    DiscontinuityIndicator = 0b00000100,
}

#[derive(Debug, PartialEq)]
pub struct EventHeader {
    pub timestamp: PravegaTimestamp,
    pub include_in_index: bool,
    pub random_access: bool,
    pub discontinuity: bool,
}

#[derive(Debug, PartialEq)]
pub struct EventWithHeader<'a> {
    pub header: EventHeader,
    pub payload: &'a [u8],
}

/**
   A struct to serialize an EventWithHeader for writing to a Pravega byte stream.

   The following encoding is used:

    0                   1                   2                   3
    0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
   +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
   |                                                               |
   |          type_code (32-bit BE signed int, set to 0)           |
   |                                                               |
   +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
   |            event_length (32-bit BE unsigned int)              |
   |    number of bytes from reserved to the end of the payload    |
   |                                                               |
   +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
   |                                                         |D|R|I|
   |                    reserved (set to 0)                  |I|A|N|
   |                                                         |S|N|D|
   +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
   |                                                               |
   |                                                               |
   |                timestamp (64-bit BE unsigned int)             |
   +               nanoseconds since 1970-01-01 00:00 TAI          +
   |                    including leap seconds                     |
   |                                                               |
   |                                                               |
   +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
   |                                                               |
   |                    payload (variable length)                  |
   |                                                               |
   +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+

   One tick mark represents one bit position.

   type code:
      The type code must be 0 which corresponds to pravega_wire_protocol::wire_commands::EventCommand.TYPE_CODE.
      This makes this byte stream compatible with a Pravega event stream reader.
   event length:
      This is number of bytes from reserved to the end of the payload.
      Encoded as a 32-bit big-endian unsigned int.
   reserved:
      All reserved bits must be 0.
      These may be utilized in the future for other purposes.
   DIS - discontinuity indicator:
      True (1) if this event is or may be discontinuous from the previous event.
      This should usually be true for the first event written by a new process.
      It has the same meaning as in an MPEG transport stream.
   RAN - random access indicator:
      True (1) when the stream may be decoded without errors from this point.
      This is also known as IDR (Instantaneous Decoder Refresh).
      Usually, MPEG I-frames will have a true value for this field and all
      other events will have a false value.
   IND - include in index:
      If true (1), this event should be included in the index.
      Typically, this will equal random_access but it is possible
      that one may want to index more often for Low-Latency HLS or
      less often to reduce the size of the index.
   timestamp:
      The timestamp counts the number of nanoseconds since the epoch 1970-01-01 00:00 TAI (International Atomic Time).
      This definition is used to avoid problems with the time going backwards during positive leap seconds.
      If the timestamp is unknown or if there is ambiguity when converting from a UTC time source
      in the vicinity of a positive leap second, timestamp can be recorded as 0.
      As of 2020-01-09, TAI is exactly 37 seconds ahead of UTC.
      This offset will change when additional leap seconds are scheduled.
      This 64-bit counter will wrap in the year 2554.
      This timestamp reflects the sampling instant of the first octet in the payload, as in RFC 3550.
      For video frames, the timestamp will reflect when the image was captured by the camera.
      If DTS can differ from PTS, this timestamp should be the PTS.
      This allows different streams to be correlated precisely.
   payload:
      Can be 0 or more MPEG TS packets, or any other payload.
      When encoding an MPEG transport stream, this is currently a single 188-byte MPEG TS packet.
      Writes of the entire frame (type code through payload) must be atomic,
      which means it must be 8 MiB or smaller.
*/
pub struct EventWriter {
}

impl EventWriter {
    pub fn new() -> Self {
        Self {}
    }

    pub fn write<'a, W>(&mut self, event: &EventWithHeader<'a>, writer: &mut W) -> Result<(), Error>
    where
        W: Write,
    {
        let mut flags = BitFlags::<EventHeaderFlags>::empty();
        if event.header.include_in_index {
            flags |= EventHeaderFlags::IncludeInIndex;
        }
        if event.header.random_access {
            flags |= EventHeaderFlags::RandomAccessIndicator;
        }
        if event.header.discontinuity {
            flags |= EventHeaderFlags::DiscontinuityIndicator;
        }
        let payload_length = event.payload.len();
        if payload_length > EventWithHeader::MAX_PAYLOAD_SIZE {
            return Err(Error::new(ErrorKind::InvalidInput, format!("Payload of {} bytes exceeds {} bytes",
                payload_length, EventWithHeader::MAX_PAYLOAD_SIZE)));
        }
        let event_length: u32 = (payload_length + 12).try_into().unwrap();
        let write_length = payload_length + 20;
        let mut bytes_to_write: Vec<u8> = vec![0; write_length];
        bytes_to_write[4..8].copy_from_slice(&event_length.to_be_bytes()[..]);
        bytes_to_write[11..12].copy_from_slice(&flags.bits().to_be_bytes()[..]);
        bytes_to_write[12..20].copy_from_slice(&event.header.timestamp.nanoseconds().unwrap_or_default().to_be_bytes()[..]);
        bytes_to_write[20..20+payload_length].copy_from_slice(&event.payload[..]);
        writer.write_all(&bytes_to_write).unwrap();
        Ok(())
    }
}

pub struct EventReader {
    // This is a copy of the first 8 bytes of the serialized EventWithHeader.
    // This currently contains only the event length but the unused bits may be used in the future.
    event_length_bytes: [u8; 8],
    // The number of bytes that follow the event length.
    event_length: usize,
    // The minimum buffer size required to read the entire EventWithHeader.
    required_buffer_length: usize,
}

// A struct to deserialize an EventWithHeader that was written to a Pravega byte stream.
impl EventReader {
    pub fn new() -> Self {
        Self {
            event_length_bytes: [0; 8],
            event_length: 0,
            required_buffer_length: 0,
        }
    }

    // Reads exactly 8 bytes from the Pravega stream, which should contain the event length.
    // Returns the minimum size of the buffer that can be passed to read_event() to read the whole event.
    pub fn read_required_buffer_length<R>(&mut self, rdr: &mut R) -> Result<usize, Error>
    where
        R: Read,
    {
        rdr.read_exact(&mut self.event_length_bytes[0..8])?;
        let event_length_bytes: [u8; 4] = self.event_length_bytes[4..8].try_into().unwrap();
        self.event_length = u32::from_be_bytes(event_length_bytes) as usize;
        // Event length must be between 12 and MAX_ATOMIC_WRITE_SIZE - 8.
        if self.event_length < 12 || 8 + self.event_length > EventWithHeader::MAX_ATOMIC_WRITE_SIZE {
            return Err(Error::new(ErrorKind::InvalidData, format!("Invalid event length {}", self.event_length)))
        }
        self.required_buffer_length = 8 + self.event_length;
        Ok(self.required_buffer_length)
    }

    // Reads the rest of event, including the rest of the EventHeader and the payload.
    // This must be called after read_required_buffer_length() has been called to determine the event length.
    // The reader must be positioned at the byte immediatley after event_length.
    pub fn read_event<'a, R>(&mut self, rdr: &mut R, buffer: &'a mut [u8]) -> Result<EventWithHeader<'a>, Error>
    where
        R: Read,
    {
        if buffer.len() < self.required_buffer_length {
            return Err(Error::new(ErrorKind::InvalidInput, "Buffer too small"))
        }
        //  Note that bytes 0..8 of buffer are unused. However, this keeps the byte ranges consistent with the writer.
        rdr.read_exact(&mut buffer[8..self.required_buffer_length])?;
        let flags = BitFlags::<EventHeaderFlags>::from_bits(buffer[11]).unwrap();
        let include_in_index = flags.contains(EventHeaderFlags::IncludeInIndex);
        let random_access = flags.contains(EventHeaderFlags::RandomAccessIndicator);
        let discontinuity = flags.contains(EventHeaderFlags::DiscontinuityIndicator);
        let timestamp = u64::from_be_bytes(buffer[12..20].try_into().unwrap());
        let timestamp = if timestamp == 0 { None } else { Some(timestamp) };
        let payload_length = self.event_length - 12;
        let payload = &buffer[20..20+payload_length];
        Ok(EventWithHeader {
            header: EventHeader {
                timestamp: PravegaTimestamp::from_nanoseconds(timestamp),
                include_in_index,
                random_access,
                discontinuity,
            },
            payload,
        })
    }
}

impl<'a> EventWithHeader<'a> {
    // Maximum size of the entire frame from type code through payload.
    // Corresponds to pravega_client_rust::event_stream_writer::EventStreamWriter.
    const MAX_ATOMIC_WRITE_SIZE: usize = 8 * 1024 * 1024;
    const MAX_PAYLOAD_SIZE: usize = EventWithHeader::MAX_ATOMIC_WRITE_SIZE - 20;

    pub fn new(payload: &'a [u8], timestamp: PravegaTimestamp,
        include_in_index: bool, random_access: bool, discontinuity: bool) -> Self {
        Self {
            header: EventHeader {
                timestamp,
                include_in_index,
                random_access,
                discontinuity,
            },
            payload: payload,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::event_serde::{EventWithHeader, EventWriter, EventReader};
    use crate::timestamp::PravegaTimestamp;
    use tracing::{info, trace};
    use rand::{RngCore, SeedableRng};
    use rand_chacha::ChaCha8Rng;
    use std::io::{Cursor, ErrorKind};

    #[test]
    fn test_event_writer_reader() {
        env_logger::init();
        let mut rng = ChaCha8Rng::seed_from_u64(1);
        for payload_length in [0, 1, 2, 188, EventWithHeader::MAX_PAYLOAD_SIZE, EventWithHeader::MAX_PAYLOAD_SIZE+1].iter() {
            info!("payload_length={}", payload_length);
            let mut payload = vec![0; *payload_length as usize];
            rng.fill_bytes(&mut payload[..]);
            let event = EventWithHeader::new(
                &payload[..],
                PravegaTimestamp::from_nanoseconds(Some(u64::MAX - 100)),
                true, true, true);
            trace!("event={:?}", event);
            // Write event
            let mut serialized_bytes_cursor = Cursor::new(vec![0 as u8; payload_length + 20]);
            let mut event_writer = EventWriter::new();
            let result = event_writer.write(&event, &mut serialized_bytes_cursor).map_err(|e| e.kind());
            if *payload_length > EventWithHeader::MAX_PAYLOAD_SIZE {
                assert_eq!(result, Err(ErrorKind::InvalidInput))
            } else {
                assert_eq!(result, Ok(()));
                serialized_bytes_cursor.set_position(0);
                trace!("serialized_bytes_cursor={:?}", serialized_bytes_cursor);
                // Read event
                let mut event_reader = EventReader::new();
                let required_buffer_length = event_reader.read_required_buffer_length(&mut serialized_bytes_cursor).unwrap();
                info!("required_buffer_length={}", required_buffer_length);
                assert_eq!(required_buffer_length, 20 + payload.len());
                let mut read_buffer: Vec<u8> = vec![0; required_buffer_length];
                let deserialized_event = event_reader.read_event(&mut serialized_bytes_cursor, &mut read_buffer[..]).unwrap();
                trace!("deserialized_event={:?}", deserialized_event);
                assert_eq!(event, deserialized_event);
            }
        }
    }
}
