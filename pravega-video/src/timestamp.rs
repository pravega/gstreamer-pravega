//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

use anyhow;
use std::convert::{TryInto, TryFrom};
use std::fmt;
use std::ops::{Add, Mul, Sub, Div};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// This stores the number of nanoseconds since the TAI epoch 1970-01-01 00:00 TAI (International Atomic Time).
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default)]
pub struct PravegaTimestamp(pub Option<u64>);

impl PravegaTimestamp {
    pub const NONE: PravegaTimestamp = PravegaTimestamp(None);
    pub const MIN: PravegaTimestamp = PravegaTimestamp(Some(0));
    pub const MAX: PravegaTimestamp = PravegaTimestamp(Some(std::u64::MAX));

    // Difference between NTP and Unix epochs.
    // Equals 70 years plus 17 leap days.
    // See [https://stackoverflow.com/a/29138806/5890553].
    const UNIX_TO_NTP_SECONDS: u64 = (70 * 365 + 17) * 24 * 60 * 60;

    // UTC to TAI offset.
    // Below is valid for dates between 2017-01-01 and the next leap second.
    // Beyond this range, we must use a table that incorporates the leap second schedule.
    // See [https://en.wikipedia.org/wiki/International_Atomic_Time].
    const UTC_TO_TAI_SECONDS: u64 = 37;

    // Create a PravegaTimestamp from the number of nanoseconds since the TAI epoch 1970-01-01 00:00:00 TAI.
    pub fn from_nanoseconds(nanoseconds: Option<u64>) -> PravegaTimestamp {
        PravegaTimestamp(nanoseconds)
    }

    /// Create a PravegaTimestamp from the number of nanoseconds since the NTP epoch 1900-01-01 00:00:00 UTC,
    /// minus leap seconds.
    /// A time that cannot be represented will return a None timestamp.
    /// TODO: Return an error if time cannot be represented.
    pub fn from_ntp_nanoseconds(nanoseconds: Option<u64>) -> PravegaTimestamp {
        match nanoseconds {
            Some(nanoseconds) => {
                if nanoseconds >= PravegaTimestamp::UNIX_TO_NTP_SECONDS * 1_000_000_000 {
                    PravegaTimestamp::from_unix_nanoseconds(Some(nanoseconds - PravegaTimestamp::UNIX_TO_NTP_SECONDS * 1_000_000_000))
                } else {
                    PravegaTimestamp(None)
                }
            },
            None => PravegaTimestamp(None),
        }
    }

    /// Create a PravegaTimestamp from the number of nanoseconds since the Unix epoch 1970-01-01 00:00:00 UTC,
    /// minus leap seconds.
    /// TODO: Return an error if time cannot be represented.
    pub fn from_unix_nanoseconds(nanoseconds: Option<u64>) -> PravegaTimestamp {
        match nanoseconds {
            Some(nanoseconds) => PravegaTimestamp(Some(nanoseconds + PravegaTimestamp::UTC_TO_TAI_SECONDS * 1_000_000_000)),
            None => PravegaTimestamp(None),
        }
    }

    pub fn now() -> PravegaTimestamp {
        PravegaTimestamp::from(SystemTime::now())
    }

    pub const fn none() -> Self {
        Self(None)
    }

    pub const fn is_some(&self) -> bool {
        matches!(self.0, Some(_))
    }

    pub const fn is_none(&self) -> bool {
        !self.is_some()
    }

    // Return the number of nanoseconds since the TAI epoch 1970-01-01 00:00:00 TAI.
    pub fn nanoseconds(&self) -> Option<u64> {
        self.0
    }

    /// TODO: Return an error if time cannot be represented.
    pub fn to_unix_nanoseconds(&self) -> Option<u64> {
        match self.nanoseconds() {
            Some(nanoseconds) => {
                if nanoseconds >= PravegaTimestamp::UTC_TO_TAI_SECONDS * 1_000_000_000 {
                    Some(nanoseconds - PravegaTimestamp::UTC_TO_TAI_SECONDS * 1_000_000_000)
                } else {
                    None
                }
            },
            None => None,
        }
    }

    pub fn to_iso_8601(&self) -> Option<String> {
        match self.nanoseconds() {
            Some(_) => {
                let system_time: SystemTime = (*self).into();
                let datetime: chrono::DateTime<chrono::offset::Utc> = system_time.into();
                let formatted_time = datetime.format("%Y-%m-%dT%T.%9fZ");
                Some(format!("{}", formatted_time))
                },
            None => None,
        }
    }

    /// Convert to format h:mm:ss.fffffffff
    /// Based on https://gstreamer.freedesktop.org/documentation/gstreamer/gstclock.html?gi-language=c#GST_STIME_ARGS.
    pub fn to_hms(&self) -> Option<String> {
        match self.nanoseconds() {
            Some(ns) => {
                const SECOND: u64 = 1_000_000_000;
                let h = ns / (SECOND * 60 * 60);
                let mm = (ns / (SECOND * 60)) % 60;
                let ss = (ns / SECOND) % 60;
                let f = ns % SECOND;
                Some(format!("{}:{:02}:{:02}.{:09}", h, mm, ss, f))
                },
            None => None,
        }
    }

    pub fn or(self, optb: PravegaTimestamp) -> PravegaTimestamp {
        match self.0 {
            Some(_) => self,
            None => optb,
        }
    }
}

// TODO: Implement TryFrom.
impl From<PravegaTimestamp> for SystemTime {
    fn from(t: PravegaTimestamp) -> SystemTime {
        match t.to_unix_nanoseconds() {
            Some(nanoseconds) => UNIX_EPOCH + Duration::from_nanos(nanoseconds),
            None => UNIX_EPOCH,
        }
    }
}

// TODO: Implement TryFrom.
impl From<SystemTime> for PravegaTimestamp {
    fn from(t: SystemTime) -> PravegaTimestamp {
        let nanoseconds: Option<u64> = match t.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos().try_into() {
            Ok(nanoseconds) => Some(nanoseconds),
            Err(_) => None,
        };
        PravegaTimestamp::from_unix_nanoseconds(nanoseconds)
    }
}

// TODO: Implement TryFrom.
impl From<Option<chrono::DateTime<chrono::Utc>>> for PravegaTimestamp {
    fn from(t: Option<chrono::DateTime<chrono::Utc>>) -> PravegaTimestamp {
        match t {
            Some(t) => PravegaTimestamp::from_unix_nanoseconds(u64::try_from(t.timestamp_nanos()).ok()),
            None => PravegaTimestamp::NONE,
        }
    }
}

impl TryFrom<Option<&str>> for PravegaTimestamp {
    type Error = anyhow::Error;

    fn try_from(t: Option<&str>) -> Result<Self, Self::Error> {
        match t {
            Some(t) => {
                let dt = chrono::DateTime::parse_from_rfc3339(t)?;
                let nanos = u64::try_from(dt.timestamp_nanos())?;
                Ok(PravegaTimestamp::from_unix_nanoseconds(Some(nanos)))
            },
            None => Ok(PravegaTimestamp::NONE),
        }
    }
}

impl TryFrom<Option<String>> for PravegaTimestamp {
    type Error = anyhow::Error;

    fn try_from(t: Option<String>) -> Result<Self, Self::Error> {
        PravegaTimestamp::try_from(t.as_deref())
    }
}

impl TryFrom<&String> for PravegaTimestamp {
    type Error = anyhow::Error;
    fn try_from(t:&String) -> Result<Self, Self::Error> {
        let dt = chrono::DateTime::parse_from_rfc3339(t)?;
        let nanos = u64::try_from(dt.timestamp_nanos())?;
        Ok(PravegaTimestamp::from_unix_nanoseconds(Some(nanos)))
    }
}

impl TryFrom<String> for PravegaTimestamp {
    type Error = anyhow::Error;
    fn try_from(t: String) -> Result<Self, Self::Error> {
        PravegaTimestamp::try_from(&t)
    }
}

/// Returns the timestamp in a friendly human-readable format.
/// This is currently the same format as to_iso_8601() but may change in the future.
/// For example: 2001-02-03T04:00:04.200000000Z
impl fmt::Display for PravegaTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self.nanoseconds() {
            Some(_) => {
                let system_time: SystemTime = (*self).into();
                let datetime: chrono::DateTime<chrono::offset::Utc> = system_time.into();
                let formatted_time = datetime.format("%Y-%m-%dT%T.%9fZ");
                f.write_fmt(format_args!("{}", formatted_time))
                },
            None => f.write_str("None"),
        }
    }
}

/// Returns the timestamp in a variety of formats useful for debugging.
/// For example: 2001-02-03T04:00:04.100000000Z (981172841100000000 ns, 272548:00:41.100000000)
impl fmt::Debug for PravegaTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self.nanoseconds() {
            Some(nanoseconds) => {
                let system_time: SystemTime = (*self).into();
                let datetime: chrono::DateTime<chrono::offset::Utc> = system_time.into();
                let formatted_time = datetime.format("%Y-%m-%dT%T.%9fZ");
                f.write_fmt(format_args!("{} ({} ns, {})", formatted_time, nanoseconds, self.to_hms().unwrap_or_default()))
                },
            None => f.write_str("None"),
        }
    }
}

impl Add for PravegaTimestamp {
    type Output = PravegaTimestamp;

    fn add(self, rhs: Self) -> Self::Output {
        match (self.0, rhs.0) {
            (Some(this), Some(rhs)) => Self(Some(this + rhs)),
            _ => Self(None),
        }
    }
}

/// This allows expressions such as "PravegaTimestamp::now() - PravegaTimestamp::now()".
impl Sub for PravegaTimestamp {
    type Output = TimeDelta;

    fn sub(self, rhs: PravegaTimestamp) -> Self::Output {
        match (self.0, rhs.0) {
            (Some(this), Some(rhs)) => TimeDelta(Some(this as i128 - rhs as i128)),
            _ => TimeDelta(None),
        }
    }
}

/// This allows expressions such as "PravegaTimestamp::now() + Duration::from_nanos(1_000_500_000)".
impl Add<Duration> for PravegaTimestamp {
    type Output = PravegaTimestamp;

    fn add(self, rhs: Duration) -> Self::Output {
        match self.0 {
            Some(this) => Self(Some(this + rhs.as_nanos() as u64)),
            _ => Self(None),
        }
    }
}

/// A time delta (difference), represented as a positive or negative number of nanoseconds.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default)]
pub struct TimeDelta(pub Option<i128>);

impl TimeDelta {
    pub fn nanoseconds(&self) -> Option<i128> {
        self.0
    }

    pub fn milliseconds(&self) -> Option<i128> {
        self.0.map(|t| t / 1000 / 1000)
    }

    pub fn seconds(&self) -> Option<i128> {
        self.0.map(|t| t / 1000 / 1000 / 1000)
    }

    /// Convert to format +h:mm:ss.fffffffff
    /// Based on https://gstreamer.freedesktop.org/documentation/gstreamer/gstclock.html?gi-language=c#GST_STIME_ARGS.
    pub fn to_hms(&self) -> Option<String> {
        match self.0 {
            Some(ns) => {
                let sign = if ns >= 0 { "+" } else { "-" };
                let ns = i128::abs(ns);
                const SEC: i128 = 1_000_000_000;
                let h = ns / (SEC * 60 * 60);
                let mm = (ns / (SEC * 60)) % 60;
                let ss = (ns / SEC) % 60;
                let f = ns % SEC;
                Some(format!("{}{}:{:02}:{:02}.{:09}", sign, h, mm, ss, f))
                },
            None => None,
        }
    }

    pub fn or(self, optb: TimeDelta) -> TimeDelta {
        match self.0 {
            Some(_) => self,
            None => optb,
        }
    }

    pub fn or_zero(self) -> TimeDelta {
        match self.0 {
            Some(_) => self,
            None => TimeDelta(Some(0)),
        }
    }
}

impl Add for TimeDelta {
    type Output = TimeDelta;

    fn add(self, rhs: Self) -> Self::Output {
        match (self.0, rhs.0) {
            (Some(this), Some(rhs)) => Self(Some(this + rhs)),
            _ => Self(None),
        }
    }
}

impl Sub for TimeDelta {
    type Output = TimeDelta;

    fn sub(self, rhs: TimeDelta) -> Self::Output {
        match (self.0, rhs.0) {
            (Some(this), Some(rhs)) => TimeDelta(Some(this - rhs)),
            _ => TimeDelta(None),
        }
    }
}

/// This allows expressions such as "time_delta / SECOND".
impl Div for TimeDelta {
    type Output = Option<i128>;

    fn div(self, rhs: TimeDelta) -> Self::Output {
        match (self.0, rhs.0) {
            (Some(this), Some(rhs)) => Some(this / rhs),
            _ => None,
        }
    }
}

/// This allows expressions such as "10 * SECOND".
impl Mul<TimeDelta> for i128 {
    type Output = TimeDelta;

    fn mul(self, rhs: TimeDelta) -> Self::Output {
        match rhs.0 {
            Some(rhs) => TimeDelta(Some(self * rhs)),
            _ => TimeDelta(None),
        }
    }
}

impl Mul<TimeDelta> for u128 {
    type Output = TimeDelta;

    fn mul(self, rhs: TimeDelta) -> Self::Output {
        match rhs.0 {
            Some(rhs) => TimeDelta(Some(self as i128 * rhs)),
            _ => TimeDelta(None),
        }
    }
}

impl Mul<TimeDelta> for u64 {
    type Output = TimeDelta;

    fn mul(self, rhs: TimeDelta) -> Self::Output {
        match rhs.0 {
            Some(rhs) => TimeDelta(Some(self as i128 * rhs)),
            _ => TimeDelta(None),
        }
    }
}

impl Mul<TimeDelta> for i32 {
    type Output = TimeDelta;

    fn mul(self, rhs: TimeDelta) -> Self::Output {
        match rhs.0 {
            Some(rhs) => TimeDelta(Some(self as i128 * rhs)),
            _ => TimeDelta(None),
        }
    }
}

impl Mul<TimeDelta> for u32 {
    type Output = TimeDelta;

    fn mul(self, rhs: TimeDelta) -> Self::Output {
        match rhs.0 {
            Some(rhs) => TimeDelta(Some(self as i128 * rhs)),
            _ => TimeDelta(None),
        }
    }
}

/// This allows expressions such as "SECOND / 10".
impl Div<i128> for TimeDelta {
    type Output = TimeDelta;

    fn div(self, rhs: i128) -> TimeDelta {
        match self.0 {
            Some(this) => TimeDelta(Some(this / rhs)),
            _ => TimeDelta(None),
        }
    }
}

impl Div<u64> for TimeDelta {
    type Output = TimeDelta;

    fn div(self, rhs: u64) -> TimeDelta {
        match self.0 {
            Some(this) => TimeDelta(Some(this / rhs as i128)),
            _ => TimeDelta(None),
        }
    }
}

impl Div<i32> for TimeDelta {
    type Output = TimeDelta;

    fn div(self, rhs: i32) -> TimeDelta {
        match self.0 {
            Some(this) => TimeDelta(Some(this / rhs as i128)),
            _ => TimeDelta(None),
        }
    }
}

/// This allows expressions such as "PravegaTimestamp::now() + SECOND".
impl Add<TimeDelta> for PravegaTimestamp {
    type Output = PravegaTimestamp;

    fn add(self, rhs: TimeDelta) -> Self::Output {
        match (self.0, rhs.0) {
            (Some(this), Some(rhs)) => Self(Some((this as i128 + rhs) as u64)),
            _ => Self(None),
        }
    }
}

/// This allows expressions such as "PravegaTimestamp::now() - SECOND".
impl Sub<TimeDelta> for PravegaTimestamp {
    type Output = PravegaTimestamp;

    fn sub(self, rhs: TimeDelta) -> Self::Output {
        match (self.0, rhs.0) {
            (Some(this), Some(rhs)) => Self(Some((this as i128 - rhs) as u64)),
            _ => Self(None),
        }
    }
}

impl fmt::Display for TimeDelta {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.write_str(&self.to_hms().unwrap_or("None".to_owned()))
    }
}

impl fmt::Debug for TimeDelta {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.write_str(&self.to_hms().unwrap_or("None".to_owned()))
    }
}

pub const DAY: TimeDelta = TimeDelta(Some(24 * 60 * 60 * 1_000_000_000));
pub const HOUR: TimeDelta = TimeDelta(Some(60 * 60 * 1_000_000_000));
pub const MINUTE: TimeDelta = TimeDelta(Some(60 * 1_000_000_000));
pub const SECOND: TimeDelta = TimeDelta(Some(1_000_000_000));
pub const MSECOND: TimeDelta = TimeDelta(Some(1_000_000));
pub const USECOND: TimeDelta = TimeDelta(Some(1_000));
pub const NSECOND: TimeDelta = TimeDelta(Some(1));

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_pravega_timestamp() {
        let s1 = "2001-02-03T04:00:00.000000000Z";
        let pt1 = PravegaTimestamp::try_from(Some(s1)).unwrap();
        println!("s1 ={}", s1);
        println!("pt1={}", pt1);
        println!("pt1={:?}", pt1);
        let s2 = pt1.to_iso_8601().unwrap();
        assert_eq!(s1, s2);

        let dpt2 = PravegaTimestamp::from_nanoseconds(Some(1_000_000_000));
        println!("dpt2={}", dpt2);
        println!("dpt2={:?}", dpt2);
        let pt3 = pt1 + dpt2;
        println!("pt3={:?}", pt3);
        assert_eq!(pt3.to_iso_8601().unwrap(), "2001-02-03T04:00:01.000000000Z");

        let dur4 = Duration::from_nanos(1_000_500_000);
        println!("dur4={:?}", dur4);
        let pt5 = pt1 + dur4;
        println!("pt5={:?}", pt5);
        assert_eq!(pt5.to_iso_8601().unwrap(), "2001-02-03T04:00:01.000500000Z");

        let delta6 = 3 * SECOND;
        println!("delta6={}", delta6);
        let pt7: PravegaTimestamp = pt1 + delta6;
        println!("pt7={:?}", pt7);
        assert_eq!(pt7.to_iso_8601().unwrap(), "2001-02-03T04:00:03.000000000Z");

        let delta8 = pt7 - pt1;
        assert_eq!(delta8.to_hms().unwrap(), "+0:00:03.000000000");
        assert_eq!(delta8, delta6);

        let delta9 = pt1 - pt7;
        assert_eq!(delta9.to_hms().unwrap(), "-0:00:03.000000000");

        let delta10 = pt1 - pt1;
        assert_eq!(delta10.to_hms().unwrap(), "+0:00:00.000000000");

        let delta11: TimeDelta = delta8 / 10;
        assert_eq!(delta11.to_hms().unwrap(), "+0:00:00.300000000");
    }
}
