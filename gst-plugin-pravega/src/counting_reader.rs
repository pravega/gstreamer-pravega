//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

#![allow(dead_code)]

use std::io::{Read, Result, Seek, SeekFrom};

/// Read adaptor that tracks the current offset so it can be returned without seeking the inner reader.
#[derive(Debug)]
pub struct CountingReader<T: Read + Seek> {
    inner: T,
    offset: u64,
}

impl<T: Read + Seek> CountingReader<T> {
    pub fn new(mut reader: T) -> Result<CountingReader<T>> {
        let offset = reader.seek(SeekFrom::Current(0))?;
        let reader = CountingReader {
            inner: reader,
            offset
        };
        Ok(reader)
    }

    /// Gets a reference to the underlying reader.
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Gets a mutable reference to the underlying reader.
    ///
    /// Care should be taken to avoid modifying the internal I/O state of the
    /// underlying reader as doing so may corrupt the offset.
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T: Read + Seek> Read for CountingReader<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        let result = self.inner.read(buf);
        if let Ok(read) = result {
            self.offset += read as u64;
        }
        result
    }
}

impl<T: Read + Seek> Seek  for CountingReader<T> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match pos {
            SeekFrom::Current(0) => Ok(self.offset),
            _ => {
                self.offset = self.inner.seek(pos)?;
                Ok(self.offset)
            }
        }
    }
}
