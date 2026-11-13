//! Explicit browser-host timezone capability using Windows calendar rules.
//! No OS settings are changed. UTC Date operations remain independent.
//! https://learn.microsoft.com/en-us/windows/win32/api/timezoneapi/
#[cfg(windows)]
mod native {
    const EPOCH_MS: i128 = 11_644_473_600_000;
    const MIN_MS: f64 = -(EPOCH_MS as f64);

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct SystemTime { year: u16, month: u16, weekday: u16, day: u16,
        hour: u16, minute: u16, second: u16, millisecond: u16 }
    #[repr(C)]
    #[derive(Default)]
    struct FileTime { low: u32, high: u32 }
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Rules { bias: i32, standard_name: [u16;32], standard_date: SystemTime,
        standard_bias: i32, daylight_name: [u16;32], daylight_date: SystemTime, daylight_bias: i32 }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Dynamic { rules: Rules, key: [u16;128], disabled: u8 }
    impl Default for Dynamic {
        fn default() -> Self {
            // POD structure: every field is an integer or integer array.
            unsafe { std::mem::zeroed() }
        }
    }
    const _: [();16] = [();std::mem::size_of::<SystemTime>()];
    const _: [();8] = [();std::mem::size_of::<FileTime>()];
    const _: [();172] = [();std::mem::size_of::<Rules>()];
    const _: [();432] = [();std::mem::size_of::<Dynamic>()];

    #[link(name="kernel32")]
    unsafe extern "system" {
        fn GetDynamicTimeZoneInformation(info: *mut Dynamic) -> u32;
        fn GetTimeZoneInformationForYear(year: u16, zone: *const Dynamic, info: *mut Rules) -> i32;
        fn FileTimeToSystemTime(input: *const FileTime, output: *mut SystemTime) -> i32;
        fn SystemTimeToFileTime(input: *const SystemTime, output: *mut FileTime) -> i32;
        fn SystemTimeToTzSpecificLocalTimeEx(zone: *const Dynamic, input: *const SystemTime, output: *mut SystemTime) -> i32;
        #[cfg(test)]
        fn SystemTimeToTzSpecificLocalTime(zone: *const Rules, input: *const SystemTime, output: *mut SystemTime) -> i32;
    }
    #[cfg(test)]
    #[link(name="advapi32")]
    unsafe extern "system" {
        fn EnumDynamicTimeZoneInformation(index: u32, info: *mut Dynamic) -> u32;
    }

    fn system_time(ms: f64) -> Result<SystemTime, String> {
        if !ms.is_finite() || ms.fract() != 0.0 || ms.abs() > 1_000_000_000_000_000.0 {
            return Err("Date is outside the Windows timezone calendar range.".into());
        }
        let ticks = ((ms as i128) + EPOCH_MS).checked_mul(10_000)
            .filter(|ticks| (0..=i64::MAX as i128).contains(ticks))
            .ok_or("Date is outside the Windows timezone calendar range.")? as u64;
        let file = FileTime { low: ticks as u32, high: (ticks >> 32) as u32 };
        let mut value = SystemTime::default();
        if unsafe { FileTimeToSystemTime(&file, &mut value) } == 0 || !(1601..=30827).contains(&value.year) {
            return Err("Windows could not represent this timezone calendar date.".into());
        }
        Ok(value)
    }

    fn wall_time(value: &SystemTime) -> Result<f64, String> {
        if !(1601..=30827).contains(&value.year) { return Err("Date is outside the Windows timezone calendar range.".into()); }
        let mut file = FileTime::default();
        if unsafe { SystemTimeToFileTime(value, &mut file) } == 0 {
            return Err("Windows could not convert this timezone calendar date.".into());
        }
        let ticks = (u64::from(file.high) << 32) | u64::from(file.low);
        if ticks % 10_000 != 0 { return Err("Windows timezone conversion returned a fractional millisecond.".into()); }
        Ok((i128::from(ticks / 10_000) - EPOCH_MS) as f64)
    }

    struct Zone {
        info: Dynamic, maximum: f64,
        #[cfg(test)]
        fixed_rules_forward: bool,
    }
    impl Zone {
        fn new(info: Dynamic) -> Result<Self, String> {
            let maximum = wall_time(&SystemTime { year:30827, month:12, day:31,
                hour:23, minute:59, second:59, millisecond:999, ..SystemTime::default() })?;
            Ok(Self { info, maximum, #[cfg(test)] fixed_rules_forward:false })
        }
        fn current() -> Result<Self, String> {
            let mut info = Dynamic::default();
            if unsafe { GetDynamicTimeZoneInformation(&mut info) } == u32::MAX {
                return Err("Windows timezone information is unavailable.".into());
            }
            Self::new(info)
        }
        fn in_range(&self, ms: f64) -> bool { ms >= MIN_MS && ms <= self.maximum && ms.fract() == 0.0 }
        fn local(&self, utc: f64) -> Result<f64, String> {
            let input = system_time(utc)?;
            let mut output = SystemTime::default();
            // The Ex API requires a registered timezone key. Explicit synthetic
            // constant-rule fixtures use the real older forward API; production
            // and default-profile tests always use the Ex snapshot path.
            #[cfg(test)]
            let success = if self.fixed_rules_forward {
                unsafe { SystemTimeToTzSpecificLocalTime(&self.info.rules, &input, &mut output) }
            } else { unsafe { SystemTimeToTzSpecificLocalTimeEx(&self.info, &input, &mut output) } };
            #[cfg(not(test))]
            let success = unsafe { SystemTimeToTzSpecificLocalTimeEx(&self.info, &input, &mut output) };
            if success == 0 {
                return Err("Windows UTC-to-local timezone conversion failed.".into());
            }
            wall_time(&output)
        }
        fn rules(&self, year: u16) -> Result<Rules, String> {
            if self.info.disabled != 0 { return Ok(self.info.rules); }
            let mut rules = Rules::default();
            if unsafe { GetTimeZoneInformationForYear(year, &self.info, &mut rules) } == 0 {
                return Err("Windows year-specific timezone rules are unavailable.".into());
            }
            Ok(rules)
        }
        fn utc(&self, local: f64) -> Result<f64, String> {
            let year = system_time(local)?.year;
            let mut years = Vec::new();
            let mut biases = Vec::new();
            for year in year.saturating_sub(1).max(1601)..=year.saturating_add(1).min(30827) {
                let rules = self.rules(year)?;
                let (standard, daylight) = rule_biases(&rules)?;
                if !biases.contains(&standard) { biases.push(standard); }
                if let Some(daylight) = daylight {
                    if !biases.contains(&daylight) { biases.push(daylight); }
                }
                years.push((year, rules));
            }
            let mut earliest: Option<f64> = None;
            for &bias in &biases {
                let utc = local + bias as f64 * 60_000.0;
                if self.in_range(utc) && self.local(utc)? == local && earliest.is_none_or(|best| utc < best) {
                    earliest = Some(utc);
                }
            }
            if let Some(utc) = earliest { return Ok(utc); }

            // A gap uses the offset immediately BEFORE the transition. Verify
            // every proposed event through the same forward mapping, including
            // year-boundary political changes; never guess from current bias.
            let mut events = Vec::new();
            for (year, rules) in years {
                let (standard, daylight) = rule_biases(&rules)?;
                if let Some(daylight) = daylight {
                    for (rule, before) in [(rules.daylight_date, standard), (rules.standard_date, daylight)] {
                        if let Some(wall) = transition(year, rule)? { events.push(wall + before as f64 * 60_000.0); }
                    }
                }
                let january = wall_time(&SystemTime { year, month:1, day:1, ..SystemTime::default() })?;
                events.push(january);
                for &bias in &biases { events.push(january + bias as f64 * 60_000.0); }
            }
            let mut result = None;
            for event in events {
                if !self.in_range(event-1.0) || !self.in_range(event) { continue; }
                let before = self.local(event-1.0)? - (event-1.0);
                let after = self.local(event)? - event;
                if after > before && local >= event + before && local < event + after {
                    let utc = local - before;
                    if !self.in_range(utc) { continue; }
                    if result.is_some_and(|old| old != utc) { return Err("Windows local-time gap mapping is ambiguous.".into()); }
                    result = Some(utc);
                }
            }
            result.ok_or_else(|| "Windows local-time timezone mapping is unavailable for this date.".into())
        }
    }

    fn rule_biases(rules: &Rules) -> Result<(i64, Option<i64>), String> {
        match (rules.standard_date.month == 0, rules.daylight_date.month == 0) {
            (true,true) => Ok((i64::from(rules.bias), None)),
            (false,false) => Ok((i64::from(rules.bias)+i64::from(rules.standard_bias),
                Some(i64::from(rules.bias)+i64::from(rules.daylight_bias)))),
            _ => Err("Windows timezone has an incomplete transition rule pair.".into()),
        }
    }

    fn transition(year: u16, mut rule: SystemTime) -> Result<Option<f64>, String> {
        if rule.month == 0 { return Ok(None); }
        if rule.year != 0 { return if rule.year == year { wall_time(&rule).map(Some) } else { Ok(None) }; }
        if !(1..=12).contains(&rule.month) || !(1..=5).contains(&rule.day) || rule.weekday > 6 {
            return Err("Windows timezone transition rule is invalid.".into());
        }
        let first = wall_time(&SystemTime { year, month:rule.month, day:1, ..SystemTime::default() })?;
        let first_weekday = system_time(first)?.weekday;
        let mut day = 1 + (rule.weekday + 7 - first_weekday) % 7 + (rule.day-1)*7;
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let last = match rule.month { 2 => if leap {29} else {28}, 4|6|9|11=>30, _=>31 };
        if day > last { day -= 7; }
        rule.year = year; rule.day = day;
        wall_time(&rule).map(Some)
    }

    pub(super) fn to_local(utc: f64) -> Result<f64, String> { Zone::current()?.local(utc) }
    pub(super) fn to_utc(local: f64) -> Result<f64, String> {
        // Every Windows bias is the sum of two signed32-bit minute fields.
        // Beyond this magnitude even the largest representable offset cannot
        // bring the result into ECMAScript's TimeClip range.
        let maximum_bias = 2.0 * 2_147_483_648.0 * 60_000.0;
        if local.is_nan() || local.abs() > 8_640_000_000_000_000.0 + maximum_bias { return Ok(f64::NAN); }
        Zone::current()?.utc(local)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        fn stamp(year:u16, month:u16, day:u16, hour:u16, minute:u16, second:u16, millisecond:u16) -> f64 {
            wall_time(&SystemTime { year, month, day, hour, minute, second, millisecond, ..SystemTime::default() }).unwrap()
        }
        fn annual(month:u16, day:u16, weekday:u16, hour:u16) -> SystemTime {
            SystemTime { month, day, weekday, hour, ..SystemTime::default() }
        }
        fn zone(bias:i32, daylight_bias:i32, daylight:SystemTime, standard:SystemTime) -> Zone {
            let mut zone = Zone::new(Dynamic { rules:Rules { bias, daylight_bias, daylight_date:daylight,
                standard_date:standard, ..Rules::default() }, disabled:1, ..Dynamic::default() }).unwrap();
            zone.fixed_rules_forward = true;
            zone
        }

        #[test]
        fn timezone_fixed_offset_and_windows_calendar_bounds_are_real() {
            let mut zone = zone(-330,0,SystemTime::default(),SystemTime::default());
            zone.info.rules.standard_bias = 123; // Ignored without transition dates.
            let utc = stamp(2024,1,1,0,0,0,7);
            let local = stamp(2024,1,1,5,30,0,7);
            assert_eq!(zone.local(utc).unwrap(), local);
            assert_eq!(zone.utc(local).unwrap(), utc);
            assert!(system_time(-8_640_000_000_000_000.0).is_err());
            assert!(system_time(8_640_000_000_000_000.0).is_err());
            assert!(system_time(0.5).is_err());
            assert!(to_utc(1e100).unwrap().is_nan());
            assert!(to_utc(-1e100).unwrap().is_nan());
        }

        #[test]
        fn timezone_northern_dst_fold_is_earliest_and_gap_uses_prior_offset() {
            let zone = zone(360,-60,annual(3,2,0,2),annual(11,1,0,2));
            let local = stamp(2024,11,3,1,30,0,7);
            let earlier = stamp(2024,11,3,6,30,0,7);
            let later = stamp(2024,11,3,7,30,0,7);
            assert_eq!(zone.local(earlier).unwrap(), local);
            assert_eq!(zone.local(later).unwrap(), local);
            assert_eq!(zone.utc(local).unwrap(), earlier);
            let gap = stamp(2024,3,10,2,30,0,7);
            let utc = stamp(2024,3,10,8,30,0,7);
            assert_eq!(zone.utc(gap).unwrap(), utc);
            assert_eq!(zone.local(utc).unwrap(), stamp(2024,3,10,3,30,0,7));
        }

        #[test]
        fn timezone_southern_and_negative_daylight_rules_keep_fold_gap_semantics() {
            let south = zone(-600,-60,annual(10,1,0,2),annual(4,1,0,3));
            assert_eq!(south.utc(stamp(2024,4,7,2,30,0,0)).unwrap(), stamp(2024,4,6,15,30,0,0));
            assert_eq!(south.utc(stamp(2024,10,6,2,30,0,0)).unwrap(), stamp(2024,10,5,16,30,0,0));
            let negative = zone(-60,60,annual(10,5,0,2),annual(3,5,0,1));
            assert_eq!(negative.utc(stamp(2024,10,27,1,30,0,0)).unwrap(), stamp(2024,10,27,0,30,0,0));
            assert_eq!(negative.utc(stamp(2024,3,31,1,30,0,0)).unwrap(), stamp(2024,3,31,1,30,0,0));
        }

        #[test]
        fn timezone_last_weekday_and_absolute_rules_keep_actual_calendar_dates() {
            assert_eq!(transition(2024,annual(2,5,0,2)).unwrap().unwrap(), stamp(2024,2,25,2,0,0,0));
            assert_eq!(transition(2025,annual(2,5,0,2)).unwrap().unwrap(), stamp(2025,2,23,2,0,0,0));
            let absolute = SystemTime { year:2024,month:3,day:10,hour:2,..SystemTime::default() };
            assert_eq!(transition(2024,absolute).unwrap().unwrap(),stamp(2024,3,10,2,0,0,0));
            assert!(transition(2025,absolute).unwrap().is_none());
        }

        #[test]
        fn timezone_current_windows_snapshot_roundtrips_real_years_without_os_changes() {
            let zone = Zone::current().unwrap();
            for year in [2010,2024,2030] {
                for month in [1,7] {
                    let utc = stamp(year,month,15,12,34,56,789);
                    let local = zone.local(utc).unwrap();
                    let inverse = zone.utc(local).unwrap();
                    assert!(inverse <= utc);
                    assert_eq!(zone.local(inverse).unwrap(),local);
                }
            }
        }

        #[test]
        fn timezone_registered_ex_snapshot_obeys_real_central_fold_and_gap() {
            let mut found = None;
            for index in 0..1024 {
                let mut info = Dynamic::default();
                let status = unsafe { EnumDynamicTimeZoneInformation(index, &mut info) };
                if status == 259 { break; }
                assert_eq!(status, 0);
                let length = info.key.iter().position(|unit| *unit == 0).unwrap_or(info.key.len());
                if String::from_utf16(&info.key[..length]).unwrap() == "Central Standard Time" {
                    found = Some(info); break;
                }
            }
            let zone = Zone::new(found.expect("Windows Central Standard Time registry entry")).unwrap();
            let local = stamp(2024,11,3,1,30,0,7);
            assert_eq!(zone.local(stamp(2024,11,3,7,30,0,7)).unwrap(),local);
            assert_eq!(zone.utc(local).unwrap(),stamp(2024,11,3,6,30,0,7));
            assert_eq!(zone.utc(stamp(2024,3,10,2,30,0,7)).unwrap(),stamp(2024,3,10,8,30,0,7));
        }
    }
}

#[cfg(windows)]
pub(super) fn to_local(utc: f64) -> Result<f64, String> { native::to_local(utc) }
#[cfg(windows)]
pub(super) fn to_utc(local: f64) -> Result<f64, String> { native::to_utc(local) }
#[cfg(not(windows))]
pub(super) fn to_local(_: f64) -> Result<f64, String> { Err("Native timezone conversion is unavailable on this platform.".into()) }
#[cfg(not(windows))]
pub(super) fn to_utc(_: f64) -> Result<f64, String> { Err("Native timezone conversion is unavailable on this platform.".into()) }
