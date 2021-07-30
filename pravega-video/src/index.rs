//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

// Module for writing and reading an index in a Pravega stream.

use crate::event_serde::EventHeaderFlags;
use crate::timestamp::PravegaTimestamp;
use crate::utils::CurrentHead;
use enumflags2::BitFlags;
use std::convert::TryInto;
use std::io::{BufReader, Error, ErrorKind, Read, Write, Seek, SeekFrom};
use tracing::{debug, trace};

pub fn get_index_stream_name(stream_name: &str) -> String {
    format!("{}-index", stream_name)
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct IndexRecord {
    pub timestamp: PravegaTimestamp,
    /// Pravega stream offset
    pub offset: u64,
    pub random_access: bool,
    pub discontinuity: bool,
}

impl IndexRecord {
    pub const RECORD_SIZE: usize = 20;

    pub fn new(timestamp: PravegaTimestamp, offset: u64,
               random_access: bool, discontinuity: bool) -> Self {
        Self {
            timestamp,
            offset,
            random_access,
            discontinuity,
        }
    }
}

/**
   A struct to serialize an IndexRecord for writing to a Pravega byte stream.

   The following encoding is used:

    0                   1                   2                   3
    0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
   +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
   |                                                         |D|R|R|
   |                    reserved (set to 0)                  |I|A|E|
   |                                                         |S|N|S|
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
   |                                                               |
   |                 offset (64-bit BE unsigned int)               |
   +                 byte offset into Pravega stream               +
   |                                                               |
   |                                                               |
   |                                                               |
   +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+

   One tick mark represents one bit position.

   reserved, RES:
      All reserved bits must be 0.
      These may be utilized in the future for other purposes.
   DIS - discontinuity indicator
   RAN - random access indicator
   timestamp:
      A timestamp value of 0 is not allowed in the index.

   See event_serde.rs for definitions of common fields.

   The index and related data stream must satisfy the following constraints.

   1. If the first record in the index has timestamp T1 and offset O1 (T1, O1),
      and the last record in the index has timestamp TN and offset TN (TN, ON),
      then the data stream can be read from offset O1 inclusive to ON exclusive.
      The bytes prior to O1 have not been truncated.
      All bytes between O1 and ON have been written to the Pravega server and,
      if written in a transaction, the transaction has been committed.
      However, it is possible that reads in this range may block for a short time
      due to processing in the Pravega server.
      Reads in this range will not block due to any delays in the writer.
   2. All events in the data stream between O1 and ON will have a timestamp
      equal to or greater than T1 and stricly less than TN.
   3. If there are no discontinuities, the samples in the stream were sampled
      beginning at time T1 and for a duration of TN - T1.
   4. If index records 2 through N have DIS of 0, then it is guaranteed that
      the bytes between O1 and ON were written continuously.
*/
pub struct IndexRecordWriter {
}

impl IndexRecordWriter {
    pub fn new() -> Self {
        Self {}
    }

    pub fn write<W>(&mut self, record: &IndexRecord, writer: &mut W) -> Result<(), Error>
    where
        W: Write,
    {
        let mut flags = BitFlags::<EventHeaderFlags>::empty();
        if record.random_access {
            flags |= EventHeaderFlags::RandomAccessIndicator;
        }
        if record.discontinuity {
            flags |= EventHeaderFlags::DiscontinuityIndicator;
        }
        let timestamp_nanos = record.timestamp.nanoseconds().unwrap_or_default();
        if timestamp_nanos == 0 {
            return Err(Error::new(ErrorKind::InvalidInput, "Timestamp is none or 0"));
        }
        let mut bytes_to_write: Vec<u8> = vec![0; IndexRecord::RECORD_SIZE];
        bytes_to_write[3..4].copy_from_slice(&flags.bits().to_be_bytes()[..]);
        bytes_to_write[4..12].copy_from_slice(&timestamp_nanos.to_be_bytes()[..]);
        bytes_to_write[12..20].copy_from_slice(&record.offset.to_be_bytes()[..]);
        writer.write(&bytes_to_write).unwrap();
        Ok(())
    }
}

pub struct IndexRecordReader {
}

// A struct to deserialize an IndexRecord that was written to a Pravega byte stream.
impl IndexRecordReader {
    pub fn new() -> Self {
        Self {}
    }

    pub fn read<R>(&mut self, rdr: &mut R) -> Result<IndexRecord, Error>
    where
        R: Read,
    {
        let mut buffer: Vec<u8> = vec![0; IndexRecord::RECORD_SIZE];
        rdr.read_exact(&mut buffer[..])?;
        let flags = BitFlags::<EventHeaderFlags>::from_bits(buffer[3]).unwrap();
        let random_access = flags.contains(EventHeaderFlags::RandomAccessIndicator);
        let discontinuity = flags.contains(EventHeaderFlags::DiscontinuityIndicator);
        let timestamp = u64::from_be_bytes(buffer[4..12].try_into().unwrap());
        // A timestamp of 0 is not allowed but if is read, it will be converted to None.
        let timestamp = if timestamp == 0 { None } else { Some(timestamp) };
        let offset = u64::from_be_bytes(buffer[12..20].try_into().unwrap());
        Ok(IndexRecord {
            timestamp: PravegaTimestamp::from_nanoseconds(timestamp),
            offset,
            random_access,
            discontinuity,
        })
    }
}

// A struct for searching an index.
// The index can be stored in any object that implements Read and Seek, including a Pravega stream.
pub struct IndexSearcher<R: Read + Seek + CurrentHead> {
    // We currently use a BufReader to improve the performance of the sequential read through the index when searching.
    reader: BufReader<R>,
}

#[derive(Debug)]
pub enum SearchMethod {
    /// If a non-exact match is found, return the index record immediately before the desired timestamp.
    Before,
    /// If a non-exact match is found, return the index record immediately after the desired timestamp.
    After,
}

impl<R: Read + Seek + CurrentHead> IndexSearcher<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader: BufReader::with_capacity(8*1024, reader),
        }
    }

    /// Returns a tuple containing an IndexRecord and index_offset.
    /// index_offset is the byte offset of this index record in the index.
    /// It can be used to truncate the index prior to the located IndexRecord.
    /// If an exact match is found, returns that index record always.
    /// If the desired size exceeds offset difference between the first and last index record in the index, returns the first index record.
    /// If the index has no records, returns an UnexpectedEof error.
    /// Otherwise, it uses the specified SearchMethod.
    /// TODO: Add flag to not consider records with random_access=false.
    /// TODO: Make this method private.
    pub fn search_size_and_return_index_offset(&mut self, size_bytes: u64, method: SearchMethod)
            -> Result<(IndexRecord, u64), Error> {

        let result = (|| {
            let mut index_record_reader = IndexRecordReader::new();

            let first_index_offset = self.reader.get_ref().current_head()?;
            let tail_offset = self.reader.seek(SeekFrom::End(0))?;
            if tail_offset < first_index_offset + IndexRecord::RECORD_SIZE as u64 {
                return Err(Error::new(ErrorKind::UnexpectedEof, "Index has no records"));
            }

            let mut last_index_offset = self.reader.seek(SeekFrom::Start(tail_offset - IndexRecord::RECORD_SIZE as u64))?;
            // TODO: Below may fail due to https://github.com/pravega/pravega-client-rust/issues/163.
            let tail_index_record = index_record_reader.read(&mut self.reader)?;

            // Read first record.
            let mut first_index_offset = self.reader.seek(SeekFrom::Start(first_index_offset))?;
            let first_index_record = index_record_reader.read(&mut self.reader)?;

            // Return first record if desired size is larger or equal to it.
            if tail_index_record.offset - first_index_record.offset <= size_bytes {
                return Ok((first_index_record, first_index_offset));
            }

            // Use binary search algorithm
            loop {
                let middle_index = (last_index_offset + first_index_offset) / 2 / IndexRecord::RECORD_SIZE as u64;
                let middle_index_offset = self.reader.seek(SeekFrom::Start(middle_index * IndexRecord::RECORD_SIZE as u64))?;
                let middle_index_record = index_record_reader.read(&mut self.reader)?;
                trace!("IndexSearcher::search_timestamp_and_return_index_offset: index_record={:?}", middle_index_record);
                if size_bytes > tail_index_record.offset - middle_index_record.offset {
                    last_index_offset = middle_index_offset - IndexRecord::RECORD_SIZE as u64;
                } else if size_bytes < tail_index_record.offset - middle_index_record.offset {
                    first_index_offset = middle_index_offset + IndexRecord::RECORD_SIZE as u64;
                } else {
                    return Ok((middle_index_record, middle_index_offset));
                }
                if first_index_offset > last_index_offset {
                    break;
                }
            }
            
            return match method {
                SearchMethod::Before => {
                    self.reader.seek(SeekFrom::Start(last_index_offset))?;
                    let last_index_record = index_record_reader.read(&mut self.reader)?;
                    Ok((last_index_record, last_index_offset))
                },
                SearchMethod::After => {
                    self.reader.seek(SeekFrom::Start(first_index_offset))?;
                    let first_index_record = index_record_reader.read(&mut self.reader)?;
                    Ok((first_index_record, first_index_offset))
                },
            }
        })();
        debug!("IndexSearcher::search_size_and_return_index_offset({}, {:?}) = {:?}", size_bytes, method, result);
        result
    }

    /// Returns a tuple containing an IndexRecord and index_offset.
    /// index_offset is the byte offset of this index record in the index.
    /// It can be used to truncate the index prior to the located IndexRecord.
    /// If an exact match is found, returns that index record always.
    /// If the desired timestamp exceeds the first and last timestamp in the index, returns the nearest index record.
    /// If the index has no records, returns an UnexpectedEof error.
    /// Otherwise, it uses the specified SearchMethod.
    /// TODO: Add flag to not consider records with random_access=false.
    /// TODO: Make this method private.
    pub fn search_timestamp_and_return_index_offset(&mut self, timestamp: PravegaTimestamp, method: SearchMethod)
            -> Result<(IndexRecord, u64), Error> {

        let result = (|| {
            let mut index_record_reader = IndexRecordReader::new();

            let first_index_offset = self.reader.get_ref().current_head()?;
            let tail_offset = self.reader.seek(SeekFrom::End(0))?;
            if tail_offset < first_index_offset + IndexRecord::RECORD_SIZE as u64 {
                return Err(Error::new(ErrorKind::UnexpectedEof, "Index has no records"));
            }

            // Get last record.
            let mut last_index_offset = self.reader.seek(SeekFrom::Start(tail_offset - IndexRecord::RECORD_SIZE as u64))?;
            // TODO: Below may fail due to https://github.com/pravega/pravega-client-rust/issues/163.
            let mut last_index_record = index_record_reader.read(&mut self.reader)?;
            // Return last record if desired timestamp is after or equal to it.
            if last_index_record.timestamp <= timestamp {
                return Ok((last_index_record, last_index_offset));
            }

            // Read first record.
            let mut first_index_offset = self.reader.seek(SeekFrom::Start(first_index_offset))?;
            let mut first_index_record = index_record_reader.read(&mut self.reader)?;
            // Return first record if desired timestamp is before or equal to it.
            if timestamp <= first_index_record.timestamp {
                return Ok((first_index_record, first_index_offset));
            }

            // Use binary search algorithm
            loop {
                let middle_index = (last_index_offset + first_index_offset) / 2 / IndexRecord::RECORD_SIZE as u64;
                let middle_index_offset = self.reader.seek(SeekFrom::Start(middle_index * IndexRecord::RECORD_SIZE as u64))?;
                let middle_index_record = index_record_reader.read(&mut self.reader)?;
                trace!("IndexSearcher::search_timestamp_and_return_index_offset: index_record={:?}", middle_index_record);
                if timestamp < middle_index_record.timestamp {
                    last_index_offset = middle_index_offset - IndexRecord::RECORD_SIZE as u64;
                } else if timestamp > middle_index_record.timestamp {
                    first_index_offset = middle_index_offset + IndexRecord::RECORD_SIZE as u64;
                } else {
                    return Ok((middle_index_record, middle_index_offset));
                }
                if first_index_offset > last_index_offset {
                    break;
                }
            }
            
            return match method {
                SearchMethod::Before => {
                    self.reader.seek(SeekFrom::Start(last_index_offset))?;
                    last_index_record = index_record_reader.read(&mut self.reader)?;
                    Ok((last_index_record, last_index_offset))
                },
                SearchMethod::After => {
                    self.reader.seek(SeekFrom::Start(first_index_offset))?;
                    first_index_record = index_record_reader.read(&mut self.reader)?;
                    Ok((first_index_record, first_index_offset))
                },
            }
        })();
        debug!("IndexSearcher::search_timestamp_and_return_index_offset({}, {:?}) = {:?}", timestamp, method, result);
        result
    }

    /// If a non-exact match is found, return the index record immediately before the desired timestamp.
    /// This is expected to be used to determine the offset at which to start reading.
    /// TODO: This should only consider index records with random_access=true.
    pub fn search_timestamp(&mut self, timestamp: PravegaTimestamp) -> Result<IndexRecord, Error> {
        let result = self.search_timestamp_and_return_index_offset(timestamp, SearchMethod::Before);
        debug!("IndexSearcher::search_timestamp({}) = {:?}", timestamp, result);
        result.map(|x| x.0)
    }

    /// If a non-exact match is found, return the index record immediately after the desired timestamp.
    /// This will consider any index record, including those with random_access=false.
    /// This is expected to be used to determine the offset at which to stop reading.
    pub fn search_timestamp_after(&mut self, timestamp: PravegaTimestamp) -> Result<IndexRecord, Error> {
        let result = self.search_timestamp_and_return_index_offset(timestamp, SearchMethod::After);
        debug!("IndexSearcher::search_timestamp_after({}) = {:?}", timestamp, result);
        result.map(|x| x.0)
    }

    /// This is expected to be used to determine the offset at which to start reading.
    /// TODO: This should only consider index records with random_access=true.
    pub fn get_first_record(&mut self) -> Result<IndexRecord, Error> {
        self.search_timestamp(PravegaTimestamp::MIN)
    }

    /// This is expected to be used to determine the offset at which to start reading.
    /// TODO: This should only consider index records with random_access=true.
    pub fn get_last_record(&mut self) -> Result<IndexRecord, Error> {
        self.search_timestamp(PravegaTimestamp::MAX)
    }

    /// Unwraps this `IndexSearcher<R>`, returning the underlying reader.
    pub fn into_inner(self) -> R {
        self.reader.into_inner()
    }

    /// Returns a list of all index records.
    /// This should only be used for debugging and testing.
    pub fn get_index_records(&mut self) -> Result<Vec<(IndexRecord, u64)>, Error> {
        let mut records = Vec::new();
        let index_begin_offset = self.reader.get_ref().current_head()?;
        let index_end_offset = self.reader.seek(SeekFrom::End(0))?;
        self.reader.seek(SeekFrom::Start(index_begin_offset))?;
        let mut index_record_reader = IndexRecordReader::new();
        let mut index_offset = index_begin_offset;
        while index_offset < index_end_offset {
            let index_record = index_record_reader.read(&mut self.reader)?;
            records.push((index_record, index_offset));
            index_offset += IndexRecord::RECORD_SIZE as u64;
        }
        Ok(records)
    }
}

#[cfg(test)]
mod test {
    use crate::index::{IndexRecord, IndexRecordWriter, IndexRecordReader, IndexSearcher, SearchMethod};
    use crate::timestamp::PravegaTimestamp;
    use tracing::info;
    use std::io::Cursor;

    #[test]
    fn test_index_writer_reader() {
        let index_record = IndexRecord::new(
            PravegaTimestamp::from_nanoseconds(Some(1_600_000_000_000_000_000)), 
            300, true, true);
        info!("index_record={:?}", index_record);
        let mut serialized_bytes_cursor = Cursor::new(vec![0 as u8; IndexRecord::RECORD_SIZE]);
        let mut index_record_writer = IndexRecordWriter::new();
        index_record_writer.write(&index_record, &mut serialized_bytes_cursor).unwrap();
        serialized_bytes_cursor.set_position(0);
        info!("serialized_bytes_cursor={:?}", serialized_bytes_cursor);

        let mut index_record_reader = IndexRecordReader::new();
        let deserialized_index_record = index_record_reader.read(&mut serialized_bytes_cursor).unwrap();
        info!("deserialized_index_record={:?}", deserialized_index_record);
        assert_eq!(index_record, deserialized_index_record);
    }

    #[test]
    fn test_index_searcher() {
        // env_logger::init();
        // Create index in memory.
        let num_recs = 100;
        let mut index_records: Vec<IndexRecord> = Vec::new();
        let mut memory_index_cursor = Cursor::new(vec![0 as u8; num_recs * IndexRecord::RECORD_SIZE]);
        let mut index_record_writer = IndexRecordWriter::new();
        let first_record = IndexRecord::new(
            PravegaTimestamp::from_nanoseconds(Some(1_600_000_000_000_000_000)),
            300, true, true);
        let mut rec = first_record;
        for i in 0..num_recs {
            info!("index_record={:?}", rec);
            index_records.push(rec);
            index_record_writer.write(&rec, &mut memory_index_cursor).unwrap();
            let timestamp = PravegaTimestamp::from_nanoseconds(Some(rec.timestamp.nanoseconds().unwrap() + 1000 + 10 * i as u64));
            rec = IndexRecord::new(
                timestamp, rec.offset + 100 + 2 * i as u64,
                true, false);
        }
        info!("index_records={:?}", index_records);
        let last_record = index_records.last().unwrap().to_owned();
        memory_index_cursor.set_position(0);
        info!("memory_index_cursor={:?}", memory_index_cursor);

        // Search index.
        let mut index_searcher = IndexSearcher::new(memory_index_cursor);

        // get_first_record
        let found_first_record = index_searcher.get_first_record().unwrap();
        info!("found_first_record={:?}", found_first_record);
        assert_eq!(found_first_record, first_record.clone());

        // get last record
        let found_last_record = index_searcher.get_last_record().unwrap();
        info!("found_last_record={:?}", found_last_record);
        assert_eq!(found_last_record, last_record);

        // Search for timestamp beyond the last record.
        let found_record_beyond_last = index_searcher.search_timestamp(
            PravegaTimestamp::from_nanoseconds(Some(last_record.timestamp.nanoseconds().unwrap() + 1))).unwrap();
        info!("found_record_beyond_last={:?}", found_record_beyond_last);
        assert_eq!(found_record_beyond_last, last_record);

        // Search for every timestamp in the index.
        for (i, rec) in index_records.iter().enumerate() {
            // Search for timestamps before and equal to the index record.
            for search_timestamp_offset in [500, 1, 0].iter() {
                let search_timestamp =
                    PravegaTimestamp::from_nanoseconds(Some(rec.timestamp.nanoseconds().unwrap() - search_timestamp_offset));
                let found_record = index_searcher.search_timestamp_and_return_index_offset(
                    search_timestamp, SearchMethod::After).unwrap();
                info!("search_timestamp={}, found_record={:?}", search_timestamp, found_record);
                assert_eq!(found_record.0, *rec);
                assert_eq!(found_record.1, (i * IndexRecord::RECORD_SIZE) as u64);
            }

            // Search for timestamps after and equal to the index record.
            for search_timestamp_offset in [500, 1, 0].iter() {
                let search_timestamp =
                    PravegaTimestamp::from_nanoseconds(Some(rec.timestamp.nanoseconds().unwrap() + search_timestamp_offset));
                let found_record = index_searcher.search_timestamp_and_return_index_offset(
                    search_timestamp, SearchMethod::Before).unwrap();
                info!("search_timestamp={}, found_record={:?}", search_timestamp, found_record);
                assert_eq!(found_record.0, *rec);
                assert_eq!(found_record.1, (i * IndexRecord::RECORD_SIZE) as u64);
            }
        }
    }
}
