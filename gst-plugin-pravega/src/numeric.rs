//
// Copyright (c) Dell Inc., or its subsidiaries. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//

use std::convert::TryFrom;

/// Compute a - b as an i64, saturating on overflow.
pub fn u64_to_i64_saturating_sub(a: u64, b: u64) -> i64 {
    if a < b {
        i64::try_from(b - a).map(|x| -x).unwrap_or(i64::MIN)
    } else {
        i64::try_from(a - b).unwrap_or(i64::MAX)
    }
}

#[cfg(test)]
mod test {
    use crate::numeric::u64_to_i64_saturating_sub;

    #[test]
    fn test_u64_to_i64_saturating_sub() {
        assert_eq!(u64_to_i64_saturating_sub(0, 0), 0);
        assert_eq!(u64_to_i64_saturating_sub(0, 1), -1);
        assert_eq!(u64_to_i64_saturating_sub(1, 0), 1);
        assert_eq!(u64_to_i64_saturating_sub(u64::MAX, 0), i64::MAX);
        assert_eq!(u64_to_i64_saturating_sub(0, u64::MAX), i64::MIN);
        assert_eq!(u64_to_i64_saturating_sub(u64::MAX, u64::MAX), 0);
        assert_eq!(u64_to_i64_saturating_sub(u64::MAX - 1, u64::MAX), -1);
        assert_eq!(u64_to_i64_saturating_sub(u64::MAX, u64::MAX - 1), 1);
        assert_eq!(u64_to_i64_saturating_sub(u64::MAX - 1, u64::MAX - 1), 0);
        assert_eq!(u64_to_i64_saturating_sub(i64::MAX as u64, 0), i64::MAX);
        assert_eq!(u64_to_i64_saturating_sub(0, i64::MAX as u64), -i64::MAX);
        assert_eq!(u64_to_i64_saturating_sub(0, i64::MAX as u64 + 1), -i64::MAX - 1);
        assert_eq!(u64_to_i64_saturating_sub(0, i64::MAX as u64 - 1), -i64::MAX + 1);
    }
}
