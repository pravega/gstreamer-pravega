
use anyhow;
use std::convert::{TryInto, TryFrom};
use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Default)]
/// This stores the number of nanoseconds since the TAI epoch 1970-01-01 00:00 TAI (International Atomic Time).
pub struct PravegaTimestamp(Option<u64>);

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

impl TryFrom<Option<String>> for PravegaTimestamp {
    type Error = anyhow::Error;

    fn try_from(t: Option<String>) -> Result<Self, Self::Error> {
        match t {            
            Some(t) => {
                let dt = chrono::DateTime::parse_from_rfc3339(&t[..])?;
                let nanos = u64::try_from(dt.timestamp_nanos())?;
                Ok(PravegaTimestamp::from_unix_nanoseconds(Some(nanos)))
            },
            None => Ok(PravegaTimestamp::NONE),
        }
    }
}

impl fmt::Display for PravegaTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self.nanoseconds() {
            Some(nanoseconds) => {
                let system_time: SystemTime = (*self).into();
                let datetime: chrono::DateTime<chrono::offset::Utc> = system_time.into();
                let formatted_time = datetime.format("%Y-%m-%dT%T.%9fZ");
                f.write_fmt(format_args!("{} ({} ns)", formatted_time, nanoseconds))
                },
            None => f.write_str("None"),
        }
    }
}

impl fmt::Debug for PravegaTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self.nanoseconds() {
            Some(nanoseconds) => {
                let system_time: SystemTime = (*self).into();
                let datetime: chrono::DateTime<chrono::offset::Utc> = system_time.into();
                let formatted_time = datetime.format("%Y-%m-%dT%T.%9fZ");
                f.write_fmt(format_args!("{} ({} ns)", formatted_time, nanoseconds))
                },
            None => f.write_str("None"),
        }
    }
}
