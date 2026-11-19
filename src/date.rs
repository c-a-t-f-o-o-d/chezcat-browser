//! Original bounded Date operations over an unforgeable scalar object slot.
//! The runtime does not consult the OS clock/timezone. Current time and supported
//! local-time operations require explicit host capabilities; no UTC substitution.
//! https://tc39.es/ecma262/multipage/numbers-and-dates.html#sec-date-objects

use super::{
    primitive_conversion::Hint, Builtin, Declaration, Fault, Host, Ordinary, Reference, Runtime,
    Value,
};

const DAY_MS: f64 = 86_400_000.0;
const MAX_TIME: f64 = 8_640_000_000_000_000.0;
const MAX_DATE_TEXT: usize = 64;
const MONTH_DAYS: [i64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
const MONTH_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const WEEK_NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

#[derive(Clone, Copy)]
pub(super) enum Op {
    Constructor,
    Now,
    Parse,
    Utc,
    Time,
    Iso,
    UtcString,
    Json,
    Primitive,
    SetTime,
    Get(&'static str),
    Set(&'static str),
    Local(&'static str),
}

impl Op {
    pub(super) fn arity(self) -> usize {
        match self {
            Self::Constructor | Self::Utc => 7,
            Self::Parse | Self::Json | Self::Primitive | Self::SetTime => 1,
            Self::Set("setUTCFullYear") | Self::Set("setUTCMinutes") => 3,
            Self::Set("setUTCHours") => 4,
            Self::Set("setUTCMonth") | Self::Set("setUTCSeconds") => 2,
            Self::Set(_) => 1,
            Self::Local("setFullYear" | "setMinutes") => 3,
            Self::Local("setHours") => 4,
            Self::Local("setMonth" | "setSeconds") => 2,
            Self::Local(name) if name.starts_with("set") => 1,
            _ => 0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Parts {
    year: i64,
    month: usize,
    day: i64,
    weekday: usize,
    hour: i64,
    minute: i64,
    second: i64,
    millisecond: i64,
}

fn local_time_fault() -> Fault {
    Fault::new("Date local-time operations are not implemented.")
}

fn clip(value: f64) -> f64 {
    if !value.is_finite() || value.abs() > MAX_TIME {
        f64::NAN
    } else {
        let value = value.trunc();
        if value == 0.0 {
            0.0
        } else {
            value
        }
    }
}

fn leap(year: i64) -> bool {
    year.rem_euclid(4) == 0 && (year.rem_euclid(100) != 0 || year.rem_euclid(400) == 0)
}

fn first_day(year: i64) -> i64 {
    365 * (year - 1970) + (year - 1969).div_euclid(4) - (year - 1901).div_euclid(100)
        + (year - 1601).div_euclid(400)
}

fn make_day(year: f64, month: f64, day: f64) -> f64 {
    if !year.is_finite() || !month.is_finite() || !day.is_finite() {
        return f64::NAN;
    }
    let month = month.trunc();
    let year = year.trunc() + (month / 12.0).floor();
    if !year.is_finite() {
        return f64::NAN;
    }
    let month = month.rem_euclid(12.0) as usize;
    let mut first = 365.0 * (year - 1970.0) + ((year - 1969.0) / 4.0).floor()
        - ((year - 1901.0) / 100.0).floor()
        + ((year - 1601.0) / 400.0).floor();
    let leap = year % 4.0 == 0.0 && (year % 100.0 != 0.0 || year % 400.0 == 0.0);
    for (index, days) in MONTH_DAYS.iter().enumerate().take(month) {
        first += (*days + i64::from(index == 1 && leap)) as f64;
    }
    // MakeDay adds the caller's integral day only after finding the month start.
    first + day.trunc() - 1.0
}

fn make_time(hour: f64, minute: f64, second: f64, millisecond: f64) -> f64 {
    if ![hour, minute, second, millisecond]
        .iter()
        .all(|n| n.is_finite())
    {
        return f64::NAN;
    }
    // The primary algorithm explicitly requires this floating-point order.
    ((hour.trunc() * 3_600_000.0 + minute.trunc() * 60_000.0) + second.trunc() * 1_000.0)
        + millisecond.trunc()
}

fn make_date(day: f64, time: f64) -> f64 {
    if !day.is_finite() || !time.is_finite() {
        return f64::NAN;
    }
    let value = day * DAY_MS + time;
    if value.is_finite() {
        value
    } else {
        f64::NAN
    }
}

fn parts(value: f64) -> Parts {
    debug_assert!(value.is_finite() && value.abs() <= 9_007_199_254_740_991.0 && value == value.trunc());
    let day = (value / DAY_MS).floor() as i64;
    // Clipped timestamps occupy fewer than 550,000 calendar years. A fixed
    // 21-step search avoids walking across centuries or input-controlled years.
    let (mut low, mut high) = (-1_000_000i64, 1_000_000i64);
    while low + 1 < high {
        let mid = low + (high - low) / 2;
        if first_day(mid) <= day {
            low = mid;
        } else {
            high = mid;
        }
    }
    let year = low;
    let mut within = day - first_day(year);
    let mut month = 0;
    while month < 11 {
        let days = MONTH_DAYS[month] + i64::from(month == 1 && leap(year));
        if within < days {
            break;
        }
        within -= days;
        month += 1;
    }
    let time = value.rem_euclid(DAY_MS) as i64;
    Parts {
        year,
        month,
        day: within + 1,
        weekday: (day + 4).rem_euclid(7) as usize,
        hour: time / 3_600_000,
        minute: time / 60_000 % 60,
        second: time / 1_000 % 60,
        millisecond: time % 1_000,
    }
}

fn iso_string(value: f64) -> Result<String, Fault> {
    if value.is_nan() {
        return Err(Fault::exception(
            "RangeError",
            "Invalid Date cannot be formatted as ISO.",
        ));
    }
    let p = parts(value);
    let year = if (0..=9999).contains(&p.year) {
        format!("{:04}", p.year)
    } else {
        format!("{}{:06}", if p.year < 0 { "-" } else { "+" }, p.year.abs())
    };
    Ok(format!(
        "{year}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        p.month + 1,
        p.day,
        p.hour,
        p.minute,
        p.second,
        p.millisecond
    ))
}

fn utc_string(value: f64) -> String {
    if value.is_nan() {
        return "Invalid Date".into();
    }
    let p = parts(value);
    let year = if p.year < 0 {
        format!("-{:04}", -p.year)
    } else {
        format!("{:04}", p.year)
    };
    format!(
        "{}, {:02} {} {year} {:02}:{:02}:{:02} GMT",
        WEEK_NAMES[p.weekday], p.day, MONTH_NAMES[p.month], p.hour, p.minute, p.second
    )
}

struct Reader<'a> {
    input: &'a [u8],
    at: usize,
}
impl Reader<'_> {
    fn take(&mut self, byte: u8) -> bool {
        if self.input.get(self.at) == Some(&byte) {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn digits(&mut self, count: usize) -> Option<i64> {
        let mut value = 0;
        for _ in 0..count {
            let digit = *self.input.get(self.at)?;
            if !digit.is_ascii_digit() {
                return None;
            }
            self.at += 1;
            value = value * 10 + i64::from(digit - b'0');
        }
        Some(value)
    }
    fn end(&self) -> bool {
        self.at == self.input.len()
    }
}

enum Parsed {
    Time(f64),
    Local,
}

fn parse_iso(source: &str) -> Option<Parsed> {
    let mut r = Reader {
        input: source.as_bytes(),
        at: 0,
    };
    let sign = if r.take(b'+') {
        1
    } else if r.take(b'-') {
        -1
    } else {
        0
    };
    let year = r.digits(if sign == 0 { 4 } else { 6 })?;
    if sign == -1 && year == 0 {
        return Some(Parsed::Time(f64::NAN));
    }
    let year = if sign == -1 { -year } else { year };
    let month = if r.take(b'-') { r.digits(2)? } else { 1 };
    let day = if r.take(b'-') { r.digits(2)? } else { 1 };
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Some(Parsed::Time(f64::NAN));
    }
    let date = make_day(year as f64, (month - 1) as f64, day as f64);
    if r.end() {
        return Some(Parsed::Time(clip(make_date(date, 0.0))));
    }
    if !r.take(b'T') {
        return None;
    }
    let hour = r.digits(2)?;
    if !r.take(b':') {
        return None;
    }
    let minute = r.digits(2)?;
    let mut second = 0;
    let mut millisecond = 0;
    if r.take(b':') {
        second = r.digits(2)?;
        if r.take(b'.') {
            millisecond = r.digits(3)?;
        }
    }
    if hour > 24
        || minute > 59
        || second > 59
        || (hour == 24 && (minute != 0 || second != 0 || millisecond != 0))
    {
        return Some(Parsed::Time(f64::NAN));
    }
    if r.end() {
        return Some(Parsed::Local);
    }
    let offset = if r.take(b'Z') {
        0
    } else {
        let sign = if r.take(b'+') {
            1
        } else if r.take(b'-') {
            -1
        } else {
            return None;
        };
        let h = r.digits(2)?;
        if !r.take(b':') {
            return None;
        }
        let m = r.digits(2)?;
        if h > 23 || m > 59 {
            return Some(Parsed::Time(f64::NAN));
        }
        sign * (h * 60 + m) * 60_000
    };
    if !r.end() {
        return None;
    }
    let time = make_time(
        hour as f64,
        minute as f64,
        second as f64,
        millisecond as f64,
    );
    Some(Parsed::Time(clip(make_date(date, time) - offset as f64)))
}

fn parse_utc_string(source: &str) -> Option<f64> {
    parse_utc_calendar(source,true)
}

fn parse_utc_calendar(source:&str, clip_result:bool) -> Option<f64> {
    let mut words = source.split(' ');
    let weekday = words.next()?;
    let day = words.next()?;
    let month = words.next()?;
    let year = words.next()?;
    let time = words.next()?;
    if words.next()? != "GMT"
        || words.next().is_some()
        || weekday.len() != 4
        || !weekday.ends_with(',')
        || day.len() != 2
        || time.len() != 8
    {
        return None;
    }
    let weekday = WEEK_NAMES.iter().position(|name| *name == &weekday[..3])?;
    let month = MONTH_NAMES.iter().position(|name| *name == month)?;
    let mut d = Reader {
        input: day.as_bytes(),
        at: 0,
    };
    let day = d.digits(2)?;
    let mut y = Reader {
        input: year.as_bytes(),
        at: 0,
    };
    let minus = y.take(b'-');
    let count = year.len() - usize::from(minus);
    if !(4..=6).contains(&count) {
        return None;
    }
    let year = y.digits(count)? * if minus { -1 } else { 1 };
    let mut t = Reader {
        input: time.as_bytes(),
        at: 0,
    };
    let hour = t.digits(2)?;
    if !t.take(b':') {
        return None;
    }
    let minute = t.digits(2)?;
    if !t.take(b':') {
        return None;
    }
    let second = t.digits(2)?;
    if day < 1
        || day > MONTH_DAYS[month] + i64::from(month == 1 && leap(year))
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    let value = make_date(
        make_day(year as f64, month as f64, day as f64),
        make_time(hour as f64, minute as f64, second as f64, 0.0),
    );
    if !value.is_finite() || value.abs()>9_007_199_254_740_991.0 || parts(value).weekday != weekday {
        return None;
    }
    let value = if clip_result {clip(value)} else {value};
    value.is_finite().then_some(value)
}

fn parse(source: &str) -> Result<f64, Fault> {
    if source.len() > MAX_DATE_TEXT || !source.is_ascii() {
        return Ok(f64::NAN);
    }
    match parse_iso(source) {
        Some(Parsed::Time(time)) => Ok(time),
        Some(Parsed::Local) => Err(local_time_fault()),
        None => Ok(parse_utc_string(source).or_else(||parse_local_string(source)).unwrap_or(f64::NAN)),
    }
}

// Parse the exact local representation emitted below, including its numeric
// offset. It has no dependency on the current host zone or current DST state.
fn parse_local_string(source:&str) -> Option<f64> {
    let mut words = source.split(' ');
    let weekday = words.next()?;
    let month = words.next()?;
    let day = words.next()?;
    let year = words.next()?;
    let time = words.next()?;
    let zone = words.next()?;
    if words.next().is_some() || zone.len()!=8 || !zone.starts_with("GMT") { return None; }
    let sign = match zone.as_bytes()[3] {b'+'=>1.0,b'-'=>-1.0,_=>return None};
    let mut offset = Reader {input:&zone.as_bytes()[4..],at:0};
    let hours = offset.digits(2)?;
    let minutes = offset.digits(2)?;
    if hours>23 || minutes>59 { return None; }
    let wall = parse_utc_calendar(&format!("{weekday}, {day} {month} {year} {time} GMT"),false)?;
    Some(clip(wall-sign*(hours*60+minutes) as f64*60_000.0))
}

impl Runtime {
    pub(super) fn initialize_date(&mut self) -> Result<(), Fault> {
        let constructor = self.native_function(Builtin::Date(Op::Constructor), "Date")?;
        let Value::Function(index) = constructor else {
            unreachable!()
        };
        self.put_binding(self.global_environment, "Date".into(), Some(constructor), Declaration::Var, false)?;
        let prototype = self.new_object(None)?;
        // Date.prototype is an ordinary object without a [[DateValue]] slot.
        self.date_prototype = Some(prototype);
        self.constructor_prototype("Date", Value::Object(prototype))?;
        let properties = self.ordinary_id(Ordinary::Function(index))?;
        for (name, op) in [("now", Op::Now), ("parse", Op::Parse), ("UTC", Op::Utc)] {
            self.prototype_method(properties, name, Builtin::Date(op))?;
        }
        for (name, op) in [
            ("getTime", Op::Time),
            ("valueOf", Op::Time),
            ("toISOString", Op::Iso),
            ("toUTCString", Op::UtcString),
            ("toJSON", Op::Json),
            ("setTime", Op::SetTime),
        ] {
            self.prototype_method(prototype, name, Builtin::Date(op))?;
        }
        let utc = self.get_property(prototype, "toUTCString")?;
        self.store_property(
            prototype,
            "toGMTString".into(),
            utc,
            Some((false, true, true)),
        )?;
        for name in [
            "getUTCFullYear",
            "getUTCMonth",
            "getUTCDate",
            "getUTCDay",
            "getUTCHours",
            "getUTCMinutes",
            "getUTCSeconds",
            "getUTCMilliseconds",
        ] {
            self.prototype_method(prototype, name, Builtin::Date(Op::Get(name)))?;
        }
        for name in [
            "setUTCFullYear",
            "setUTCMonth",
            "setUTCDate",
            "setUTCHours",
            "setUTCMinutes",
            "setUTCSeconds",
            "setUTCMilliseconds",
        ] {
            self.prototype_method(prototype, name, Builtin::Date(Op::Set(name)))?;
        }
        for name in [
            "getFullYear",
            "getMonth",
            "getDate",
            "getDay",
            "getHours",
            "getMinutes",
            "getSeconds",
            "getMilliseconds",
            "getTimezoneOffset",
            "getYear",
            "setFullYear",
            "setMonth",
            "setDate",
            "setHours",
            "setMinutes",
            "setSeconds",
            "setMilliseconds",
            "setYear",
            "toString",
            "toDateString",
            "toTimeString",
            "toLocaleString",
            "toLocaleDateString",
            "toLocaleTimeString",
        ] {
            self.prototype_method(prototype, name, Builtin::Date(Op::Local(name)))?;
        }
        let primitive =
            self.native_function(Builtin::Date(Op::Primitive), "[Symbol.toPrimitive]")?;
        self.store_symbol_property(
            prototype,
            self.symbol_to_primitive.unwrap(),
            primitive,
            Some((false, false, true)),
        )
    }

    fn date_clock(&mut self, host: &mut dyn Host) -> Result<f64, Fault> {
        let time = host.date_now().map_err(Fault::new)?;
        if !time.is_finite() || time.abs() > MAX_TIME || time != time.trunc() {
            return Err(Fault::new(
                "Date clock capability returned an invalid millisecond timestamp.",
            ));
        }
        Ok(if time == 0.0 { 0.0 } else { time })
    }

    pub(super) fn date_construct(
        &mut self,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        let value = match args {
            [] => self.date_clock(host)?,
            [value] => {
                if let Value::Object(id) = value {
                    if let Some(time) = self.object(*id)?.date {
                        return self.date_object(time);
                    }
                }
                let input = self.copy_value(value)?;
                let primitive = self.primitive_with_preference(input, Hint::Default, host)?;
                match primitive {
                    Value::String(text) => {
                        self.work(text.len())?;
                        parse(&text)?
                    }
                    other => clip(other.script_number()?),
                }
            }
            _ => {
                let calendar = self.date_calendar(args, host)?;
                // Local timezone offsets cannot bring a value more than a
                // day outside the Date range back into the clipped range.
                if calendar.abs() > MAX_TIME + DAY_MS { f64::NAN }
                else { self.date_from_local_calendar(calendar, host)? }
            }
        };
        self.date_object(value)
    }

    fn date_object(&mut self, time: f64) -> Result<Value, Fault> {
        let object = self.new_object(None)?;
        self.objects[object].date = Some(time);
        self.objects[object].prototype = self.date_prototype.map(Ordinary::Object);
        Ok(Value::Object(object))
    }

    fn date_receiver(&self, value: &Value) -> Result<(usize, f64), Fault> {
        if let Value::Object(id) = value {
            if let Some(time) = self.object(*id)?.date {
                return Ok((*id, time));
            }
        }
        Err(Fault::exception(
            "TypeError",
            "Date method has an incompatible receiver.",
        ))
    }

    pub(super) fn date_builtin(
        &mut self,
        op: Op,
        args: &[Value],
        this: Value,
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        self.step()?;
        self.check_native_call_stack("Date callback call")?;
        self.call_depth += 1;
        // Calendar and formatting work is constant-bounded; callbacks retain
        // their existing execution/copy limits and cannot start a fresh job.
        let result = (|| {
            self.work(64)?;
            match op {
                Op::Constructor => {
                    let now = self.date_clock(host)?;
                    self.date_local_string(now,"toString",host)
                }
                Op::Now => self.date_clock(host).map(Value::Number),
                Op::Parse => {
                    let input = self.copy_value(args.first().unwrap_or(&Value::Undefined))?;
                    let value = self.primitive_with_preference(input, Hint::String, host)?;
                    let text = value.script_text()?;
                    self.work(text.len())?;
                    parse(&text).map(Value::Number)
                }
                Op::Utc => self.date_utc(args, host).map(Value::Number),
                Op::Json => self.date_json(this, host),
                Op::Primitive => self.date_primitive(this, args, host),
                _ => {
                    let (object, time) = self.date_receiver(&this)?;
                    match op {
                        Op::Time => Ok(Value::Number(time)),
                        Op::Iso => self.checked_value(Value::String(iso_string(time)?)),
                        Op::UtcString => self.checked_value(Value::String(utc_string(time))),
                        Op::Get(name) => Ok(Value::Number(if time.is_nan() {
                            f64::NAN
                        } else {
                            let p = parts(time);
                            match name {
                                "getUTCFullYear" => p.year as f64,
                                "getUTCMonth" => p.month as f64,
                                "getUTCDate" => p.day as f64,
                                "getUTCDay" => p.weekday as f64,
                                "getUTCHours" => p.hour as f64,
                                "getUTCMinutes" => p.minute as f64,
                                "getUTCSeconds" => p.second as f64,
                                "getUTCMilliseconds" => p.millisecond as f64,
                                _ => unreachable!(),
                            }
                        })),
                        Op::SetTime => {
                            let value =
                                clip(self.binary_number(
                                    args.first().unwrap_or(&Value::Undefined),
                                    host,
                                )?);
                            self.objects[object].date = Some(value);
                            Ok(Value::Number(value))
                        }
                        Op::Set(name) => self
                            .date_set_utc(object, time, name, args, host)
                            .map(Value::Number),
                        Op::Local(name) => {
                            if time.is_nan() && name.starts_with("get") {
                                return Ok(Value::Number(f64::NAN));
                            }
                            if matches!(name, "getSeconds" | "getMilliseconds" | "getMinutes" | "getHours" | "getTimezoneOffset") {
                                let local = self.date_local_time(time, host)?;
                                let value = match name {
                                    "getSeconds" => (local.rem_euclid(60_000.0) / 1000.0).floor(),
                                    "getMilliseconds" => local.rem_euclid(1000.0),
                                    "getMinutes" => (local.rem_euclid(3_600_000.0) / 60_000.0).floor(),
                                    "getHours" => (local.rem_euclid(DAY_MS) / 3_600_000.0).floor(),
                                    "getTimezoneOffset" => (time-local) / 60_000.0,
                                    _ => unreachable!(),
                                };
                                return Ok(Value::Number(if value == 0.0 { 0.0 } else { value }));
                            }
                            if name == "setSeconds" {
                                return self.date_set_local_seconds(object, time, args, host).map(Value::Number);
                            }
                            if time.is_nan()
                                && matches!(name, "toString" | "toDateString" | "toTimeString")
                            {
                                return self.checked_value(Value::String("Invalid Date".into()));
                            }
                            if matches!(name,"toString"|"toDateString"|"toTimeString") {
                                return self.date_local_string(time,name,host);
                            }
                            Err(Fault::new(format!("Date local-time operation {name} is not implemented.")))
                        }
                        _ => unreachable!(),
                    }
                }
            }
        })();
        self.call_depth -= 1;
        result
    }

    fn date_local_time(&mut self, time: f64, host: &mut dyn Host) -> Result<f64, Fault> {
        let local = host.date_to_local(time).map_err(Fault::new)?;
        if !local.is_finite() || local.fract() != 0.0 || local.abs() > 9_007_199_254_740_991.0 {
            return Err(Fault::new("Host timezone did not return exact finite calendar milliseconds."));
        }
        Ok(local)
    }

    fn date_local_string(&mut self, time:f64, name:&str, host:&mut dyn Host) -> Result<Value,Fault> {
        if time.is_nan() { return self.checked_value(Value::String("Invalid Date".into())); }
        let local = self.date_local_time(time,host)?;
        let p = parts(local);
        let year = if p.year<0 {format!("-{:04}",-p.year)} else {format!("{:04}",p.year)};
        let date = format!("{} {} {:02} {year}",WEEK_NAMES[p.weekday],MONTH_NAMES[p.month],p.day);
        if name=="toDateString" { return self.checked_value(Value::String(date)); }
        let offset = local-time;
        let minutes = ((offset.abs()/60_000.0).floor() as i64)%60;
        let hours = ((offset.abs()/3_600_000.0).floor() as i64)%24;
        // The standard permits an empty implementation-defined zone name.
        // The numeric offset always describes this instant, including DST.
        let clock = format!("{:02}:{:02}:{:02} GMT{}{:02}{:02}",p.hour,p.minute,p.second,
            if offset>=0.0 {'+'} else {'-'},hours,minutes);
        self.checked_value(Value::String(if name=="toTimeString" {clock} else {format!("{date} {clock}")}))
    }

    fn date_set_local_seconds(&mut self, object: usize, time: f64, args: &[Value], host: &mut dyn Host) -> Result<f64, Fault> {
        // Snapshot the old slot before conversions, preserving callback effects
        // on invalid dates and on later host conversion failures.
        let second = self.binary_number(args.first().unwrap_or(&Value::Undefined), host)?;
        let millisecond = args.get(1).map(|value| self.binary_number(value, host)).transpose()?;
        if time.is_nan() { return Ok(f64::NAN); }
        let local = self.date_local_time(time, host)?;
        let within = local.rem_euclid(DAY_MS);
        let result = make_date((local/DAY_MS).floor(), make_time(
            (within/3_600_000.0).floor(), (within.rem_euclid(3_600_000.0)/60_000.0).floor(),
            second, millisecond.unwrap_or(local.rem_euclid(1000.0))));
        let value = self.date_from_local_calendar(result, host)?;
        self.objects[object].date = Some(value);
        Ok(value)
    }

    fn date_from_local_calendar(&self, calendar: f64, host: &mut dyn Host) -> Result<f64, Fault> {
        if !calendar.is_finite() { return Ok(f64::NAN); }
        let utc = host.date_to_utc(calendar).map_err(Fault::new)?;
        if utc.is_infinite() || utc.is_finite() && utc.fract() != 0.0 {
            return Err(Fault::new("Host timezone did not return integral UTC milliseconds."));
        }
        Ok(clip(utc))
    }

    fn date_utc(&mut self, args: &[Value], host: &mut dyn Host) -> Result<f64, Fault> {
        self.date_calendar(args, host).map(clip)
    }

    fn date_calendar(&mut self, args: &[Value], host: &mut dyn Host) -> Result<f64, Fault> {
        let mut values = [f64::NAN, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0];
        for (index, value) in args.iter().take(7).enumerate() {
            values[index] = self.binary_number(value, host)?;
        }
        let year = values[0].trunc();
        values[0] = if (0.0..=99.0).contains(&year) {
            year + 1900.0
        } else {
            year
        };
        Ok(make_date(
            make_day(values[0], values[1], values[2]),
            make_time(values[3], values[4], values[5], values[6]),
        ))
    }

    fn date_set_utc(
        &mut self,
        object: usize,
        mut time: f64,
        name: &'static str,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<f64, Fault> {
        // Capture the old date before conversions, which may mutate this same
        // object's slot. An invalid non-year setter returns without overwriting
        // such callback mutations; it still converts every supplied argument.
        if name == "setUTCFullYear" && time.is_nan() {
            time = 0.0;
        }
        let mut converted = [f64::NAN; 4];
        let count = args.len().min(Op::Set(name).arity()).max(1);
        for (index, target) in converted.iter_mut().enumerate().take(count) {
            *target = self.binary_number(args.get(index).unwrap_or(&Value::Undefined), host)?;
        }
        if time.is_nan() {
            return Ok(f64::NAN);
        }
        let p = parts(time);
        let get = |index, fallback| {
            if index < count {
                converted[index]
            } else {
                fallback
            }
        };
        let day = (time / DAY_MS).floor();
        let within = time.rem_euclid(DAY_MS);
        let value = match name {
            "setUTCFullYear" => make_date(
                make_day(converted[0], get(1, p.month as f64), get(2, p.day as f64)),
                within,
            ),
            "setUTCMonth" => make_date(
                make_day(p.year as f64, converted[0], get(1, p.day as f64)),
                within,
            ),
            "setUTCDate" => make_date(
                make_day(p.year as f64, p.month as f64, converted[0]),
                within,
            ),
            "setUTCHours" => make_date(
                day,
                make_time(
                    converted[0],
                    get(1, p.minute as f64),
                    get(2, p.second as f64),
                    get(3, p.millisecond as f64),
                ),
            ),
            "setUTCMinutes" => make_date(
                day,
                make_time(
                    p.hour as f64,
                    converted[0],
                    get(1, p.second as f64),
                    get(2, p.millisecond as f64),
                ),
            ),
            "setUTCSeconds" => make_date(
                day,
                make_time(
                    p.hour as f64,
                    p.minute as f64,
                    converted[0],
                    get(1, p.millisecond as f64),
                ),
            ),
            "setUTCMilliseconds" => make_date(
                day,
                make_time(
                    p.hour as f64,
                    p.minute as f64,
                    p.second as f64,
                    converted[0],
                ),
            ),
            _ => unreachable!(),
        };
        let value = clip(value);
        self.objects[object].date = Some(value);
        Ok(value)
    }

    fn date_json(&mut self, this: Value, host: &mut dyn Host) -> Result<Value, Fault> {
        let object = match this {
            Value::Undefined | Value::Null => {
                return Err(Fault::exception(
                    "TypeError",
                    "Date.toJSON needs an object-coercible receiver.",
                ))
            }
            Value::Object(_) | Value::Function(_) | Value::Host(_) => this,
            _ => self.box_primitive(this)?,
        };
        let primitive = self.primitive_with_preference(object.clone(), Hint::Number, host)?;
        if matches!(primitive, Value::Number(value) if !value.is_finite()) {
            return Ok(Value::Null);
        }
        let method = self.read_reference(
            &Reference::Property(object.clone(), "toISOString".into()),
            host,
        )?;
        self.call_value(method, &[], object, host)
    }

    fn date_primitive(
        &mut self,
        this: Value,
        args: &[Value],
        host: &mut dyn Host,
    ) -> Result<Value, Fault> {
        if matches!(this, Value::Host(handle) if !host.primitive_conversion(handle)) {
            return Err(Fault::new(
                "Date primitive conversion of this native host is not implemented.",
            ));
        }
        if !matches!(this, Value::Object(_) | Value::Function(_) | Value::Host(_)) {
            return Err(Fault::exception(
                "TypeError",
                "Date primitive conversion needs an object receiver.",
            ));
        }
        let hint = match args.first() {
            Some(Value::String(hint)) if matches!(hint.as_str(), "default" | "string") => true,
            Some(Value::String(hint)) if hint == "number" => false,
            _ => {
                return Err(Fault::exception(
                    "TypeError",
                    "Date primitive conversion received an invalid hint.",
                ))
            }
        };
        self.primitive_ordinary(this, hint, host)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoHost;
    impl Host for NoHost {
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
            Err("No host read".into())
        }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
            Err("No host write".into())
        }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
            Err("No host call".into())
        }
    }
    fn succeeds(source: &str) {
        assert_eq!(
            Runtime::for_test()
                .execute(source, &mut NoHost)
                .unwrap()
                .to_text(),
            "ok"
        );
    }

    #[test]
    fn dates_timestamp_clipping_and_full_range_iso() {
        succeeds(
            r#"
if(new Date(0).toISOString()!=='1970-01-01T00:00:00.000Z')throw 'epoch';
if(new Date(-1.9).toISOString()!=='1969-12-31T23:59:59.999Z')throw 'negative';
if(new Date(8640000000000000).toISOString()!=='+275760-09-13T00:00:00.000Z')throw 'max';
if(new Date(-8640000000000000).toISOString()!=='-271821-04-20T00:00:00.000Z')throw 'min';
if(!isNaN(new Date(8640000000000001).getTime())||1/new Date(-0.9).getTime()!==Infinity)throw 'clip';
var caught=false;try{new Date(NaN).toISOString();}catch(e){caught=e.name==='RangeError';}if(!caught)throw 'invalid iso';'ok';
"#,
        );
    }

    #[test]
    fn dates_iso_offsets_defaults_and_invalid_fields() {
        succeeds(
            r#"
if(new Date('2024').toISOString()!=='2024-01-01T00:00:00.000Z')throw 'defaults';
if(new Date('2024-02T12:34Z').toISOString()!=='2024-02-01T12:34:00.000Z')throw 'month time';
if(new Date('2024-02-29T12:34:56.789+05:30').toISOString()!=='2024-02-29T07:04:56.789Z')throw 'offset';
if(new Date('2024-02-29T24:00Z').toISOString()!=='2024-03-01T00:00:00.000Z')throw '24';
for(var s of ['-000000-01-01T00:00Z','2024-13-01','2024-01-32','2024-01-01T24:01Z','2024-01-01T01:00+24:00'])if(!isNaN(Date.parse(s)))throw 'invalid';
'ok';
"#,
        );
    }

    #[test]
    fn dates_utc_normalization_centuries_and_zero_year() {
        succeeds(
            r#"
if(new Date(Date.UTC(0,0)).toISOString()!=='1900-01-01T00:00:00.000Z')throw 'short year';
if(new Date(Date.UTC(2000,1,30)).toISOString()!=='2000-03-01T00:00:00.000Z')throw 'leap';
if(new Date(Date.UTC(1900,1,29)).toISOString()!=='1900-03-01T00:00:00.000Z')throw 'century';
if(new Date(Date.UTC(1970,-1,1)).toISOString()!=='1969-12-01T00:00:00.000Z')throw 'month';
var d=new Date('0000-01-01T00:00Z');if(d.getUTCFullYear()!==0||d.getUTCDay()!==6)throw 'year zero';
if(!isNaN(Date.UTC())||Date.UTC(1970)!==0)throw 'arity';'ok';
"#,
        );
    }

    #[test]
    fn dates_brands_copy_hooks_and_frozen_slots() {
        succeeds(
            r#"
var d=new Date(42);Object.defineProperty(d,Symbol.toPrimitive,{get:function(){throw 'hook';}});d.valueOf=function(){throw 'value';};
Object.setPrototypeOf(d,null);if(new Date(d).getTime()!==42)throw 'copy';
var n=new Date(0);Object.freeze(n);if(n.setTime(9)!==9||n.getTime()!==9)throw 'frozen slot';
if(Object.prototype.toString.call(n)!=='[object Date]'||Object.prototype.toString.call(Date.prototype)!=='[object Object]')throw 'tags';
var reads=0,caught=0;try{Date.prototype.setTime.call({}, {valueOf:function(){reads++;return 0;}});}catch(e){if(e.name==='TypeError')caught++;}
try{Date.prototype.getTime();}catch(e){if(e.name==='TypeError')caught++;}if(reads||caught!==2)throw 'branding';'ok';
"#,
        );
    }

    #[test]
    fn dates_utc_setters_capture_state_and_preserve_invalid_callback_mutation() {
        succeeds(
            r#"
var d=new Date('2000-01-02T03:04:05.006Z');d.setUTCSeconds({valueOf:function(){d.setTime(0);return 8;}});
if(d.toISOString()!=='2000-01-02T03:04:08.006Z')throw 'snapshot';
d.setTime(NaN);var result=d.setUTCMonth({valueOf:function(){d.setTime(17);return 1;}});
if(!isNaN(result)||d.getTime()!==17)throw 'invalid callback';
d.setTime(NaN);d.setUTCFullYear(0,1,29);if(d.toISOString()!=='0000-02-29T00:00:00.000Z')throw 'revival';
d.setUTCMonth(14,3);if(d.toISOString()!=='0001-03-03T00:00:00.000Z')throw 'rollover';'ok';
"#,
        );
    }

    #[test]
    fn dates_json_generic_lookup_and_primitive_protocol() {
        succeeds(
            r#"
if(JSON.stringify([new Date(0),new Date(NaN)])!=='["1970-01-01T00:00:00.000Z",null]')throw 'json';
var log='',o={};o[Symbol.toPrimitive]=function(h){log+=h;return 'text';};Object.defineProperty(o,'toISOString',{get:function(){log+='g';return function(){if(this!==o)throw 'this';log+='c';return 7;};}});
if(Date.prototype.toJSON.call(o)!==7||log!=='numbergc')throw 'generic';
var f=Date.prototype[Symbol.toPrimitive],x={toString:function(){return 's';},valueOf:function(){return 2;}};
if(f.call(x,'default')!=='s'||f.call(x,'number')!==2)throw 'hints';
var caught=false;try{f.call(x,{toString:function(){throw 'must not convert';}});}catch(e){caught=e.name==='TypeError';}if(!caught)throw 'hint type';'ok';
"#,
        );
    }

    #[test]
    fn dates_utc_string_roundtrips_and_legacy_alias_identity() {
        succeeds(
            r#"
var d=new Date('2000-02-29T12:34:56Z');if(d.toUTCString()!=='Tue, 29 Feb 2000 12:34:56 GMT')throw 'utc';
if(Date.parse(d.toUTCString())!==d.getTime()||Date.prototype.toGMTString!==Date.prototype.toUTCString)throw 'roundtrip';
if(new Date(NaN).toUTCString()!=='Invalid Date'||new Date(NaN).toString()!=='Invalid Date')throw 'invalid';'ok';
"#,
        );
    }

    #[test]
    fn dates_clock_is_explicit_default_deny_and_fatal() {
        for source in ["Date.now();", "new Date();", "Date();"] {
            let mut runtime = Runtime::for_test();
            runtime
                .execute("var caught=false,finished=false;", &mut NoHost)
                .unwrap();
            let error = runtime
                .execute(
                    &format!("try{{{source}}}catch(e){{caught=true;}}finally{{finished=true;}}"),
                    &mut NoHost,
                )
                .unwrap_err();
            assert!(error.contains("clock"), "{error}");
            runtime.begin_job();
            assert_eq!(
                runtime
                    .execute("!caught&&!finished;", &mut NoHost)
                    .unwrap()
                    .to_text(),
                "true"
            );
        }
    }

    #[test]
    fn dates_host_clock_is_read_once_and_validated() {
        struct Clock {
            now: f64,
            reads: usize,
        }
        impl Host for Clock {
            fn date_now(&mut self) -> Result<f64, String> {
                self.reads += 1;
                Ok(self.now)
            }
            fn get(&mut self, _: u32, _: &str) -> Result<Value, String> {
                Err("No read".into())
            }
            fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> {
                Err("No write".into())
            }
            fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> {
                Err("No call".into())
            }
        }
        let mut clock = Clock {
            now: 1_700_000_000_123.0,
            reads: 0,
        };
        let mut runtime = Runtime::for_test();
        assert_eq!(
            runtime
                .execute("Date.now();", &mut clock)
                .unwrap()
                .to_number(),
            clock.now
        );
        assert_eq!(
            runtime
                .execute("new Date().getTime();", &mut clock)
                .unwrap()
                .to_number(),
            clock.now
        );
        assert_eq!(clock.reads, 2);
        runtime
            .execute("new Date(0);Date.parse('1970');Date.UTC(1970);", &mut clock)
            .unwrap();
        assert_eq!(clock.reads, 2);
        for value in [f64::NAN, f64::INFINITY, MAX_TIME + 1.0, 0.5] {
            clock.now = value;
            runtime.begin_job();
            assert!(runtime
                .execute("Date.now();", &mut clock)
                .unwrap_err()
                .contains("clock"));
        }
    }

    #[test]
    fn dates_local_time_capabilities_never_silently_substitute_utc() {
        for source in [
            "new Date(2024,0);",
            "Date.parse('2024-01-01T12:00');",
            "new Date(0).getFullYear();",
            "new Date(0).getSeconds();",
            "new Date(0).setSeconds(12);",
            "String(new Date(0));",
            "new Date(0).setHours(12);",
        ] {
            let error = Runtime::for_test().execute(source, &mut NoHost).unwrap_err();
            assert!(error.contains("local-time"), "{source}: {error}");
        }
    }

    #[derive(Default)]
    struct LocalHost {
        offset: f64,
        central: bool,
        fail_inverse: bool,
        bad_local: Option<f64>,
        bad_utc: Option<f64>,
        reads: usize,
        writes: usize,
    }
    impl Host for LocalHost {
        fn date_now(&mut self) -> Result<f64,String> { Ok(0.0) }
        fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { Err("No read".into()) }
        fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { Err("No write".into()) }
        fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { Err("No call".into()) }
        fn date_to_local(&mut self, utc: f64) -> Result<f64, String> {
            self.reads += 1;
            let offset = if self.central {
                if utc >= parse("2024-03-10T08:00Z").unwrap() && utc < parse("2024-11-03T07:00Z").unwrap() { -5.0*3_600_000.0 } else { -6.0*3_600_000.0 }
            } else { self.offset };
            Ok(self.bad_local.unwrap_or(utc + offset))
        }
        fn date_to_utc(&mut self, local: f64) -> Result<f64, String> {
            self.writes += 1;
            if self.fail_inverse { return Err("Timezone inverse unavailable".into()); }
            // Explicit 2024 Central wall-time fixture: spring gap uses the old
            // offset; the autumn repeated hour uses its earlier occurrence.
            let offset = if self.central {
                if local >= parse("2024-03-10T03:00Z").unwrap() && local < parse("2024-11-03T02:00Z").unwrap() { -5.0*3_600_000.0 } else { -6.0*3_600_000.0 }
            } else { self.offset };
            Ok(self.bad_utc.unwrap_or(local - offset))
        }
    }
    fn local_succeeds(source: &str, host: &mut LocalHost) {
        assert_eq!(Runtime::for_test().execute(source, host).unwrap().to_text(), "ok");
    }

    #[test]
    fn dates_local_calendar_constructor_coerces_normalizes_and_uses_the_host_inverse() {
        let mut host = LocalHost { offset: 19_800_000.0, ..LocalHost::default() };
        local_succeeds(r#"
var order='';
function argument(name, value){return {valueOf:function(){order+=name;return value;}};}
var date=new Date(argument('y',99.9),argument('m',13),argument('d',0),
    argument('h',25.5),argument('n',2),argument('s',3),argument('x',4),argument('unused',5));
if(order!=='ymdhnsx'||date.toISOString()!=='2000-01-31T19:32:03.004Z')
    throw 'coercion or local calendar';
if(new Date(1970,1).toUTCString()!=='Sat, 31 Jan 1970 18:30:00 GMT')
    throw 'local defaults';
if(!isNaN(new Date(1970,undefined))||!isNaN(new Date(1970,1,undefined))||
   !isNaN(new Date(1e8,0)))throw 'invalid calendar';
var marker={};order='';
try{new Date(argument('a',NaN),{valueOf:function(){throw marker;}},argument('bad',1));}
catch(error){if(error!==marker||order!=='a')throw 'abrupt conversion';}
if(Date.UTC(1970,1)!==Date.parse('1970-02-01T00:00:00Z'))throw 'UTC calendar changed';'ok';
"#, &mut host);
        assert_eq!((host.reads, host.writes), (0, 2));
    }

    #[test]
    fn dates_local_calendar_constructor_keeps_fold_and_gap_disambiguation() {
        let mut host = LocalHost { central: true, ..LocalHost::default() };
        local_succeeds(r#"
if(new Date(2024,10,3,1,30).toISOString()!=='2024-11-03T06:30:00.000Z')
    throw 'fold earlier';
if(new Date(2024,2,10,2,30).toISOString()!=='2024-03-10T08:30:00.000Z')
    throw 'gap earlier offset';'ok';
"#, &mut host);
        assert_eq!(host.writes, 2);
    }

    #[test]
    fn dates_local_seconds_reflect_offset_rollover_truncation_and_positive_zero() {
        let mut host = LocalHost { offset: 19_800_000.0, ..LocalHost::default() };
        local_succeeds(r#"
var d=new Date('2024-01-01T00:00:58.007Z');
if(d.getHours()!==5||d.getMinutes()!==30||d.getSeconds()!==58||d.getMilliseconds()!==7||d.getTimezoneOffset()!==-330)throw 'local components';
if(d.setSeconds(61.9,123.9)!==Date.parse('2024-01-01T00:01:01.123Z'))throw 'truncate/rollover';
if(d.setSeconds(-1)!==Date.parse('2024-01-01T00:00:59.123Z'))throw 'negative rollover';
for(var arg of [NaN,Infinity,1e100]){d.setTime(0);if(!isNaN(d.setSeconds(arg))||!isNaN(d.getTime()))throw 'invalid seconds';}
d.setTime(0);if(!isNaN(d.setSeconds(1,undefined)))throw 'explicit undefined ms';
d.setTime(0);if(!isNaN(d.setSeconds()))throw 'missing seconds';'ok';
"#, &mut host);
        let mut zero = LocalHost::default();
        local_succeeds(r#"
var d=new Date(-86400000);
for(var method of ['getSeconds','getMilliseconds','getMinutes','getHours','getTimezoneOffset'])if(!Object.is(d[method](),0))throw 'negative zero';'ok';
"#, &mut zero);
    }

    #[test]
    fn dates_local_seconds_preserve_snapshot_callback_order_and_exception_identity() {
        let mut host = LocalHost { offset: 19_800_000.0, ..LocalHost::default() };
        local_succeeds(r#"
var d=new Date('2000-01-02T03:04:05.006Z'),log='';
d.setSeconds({valueOf:function(){log+='s';d.setTime(0);return 8;}},{valueOf:function(){log+='m';return 9;}});
if(log!=='sm'||d.toISOString()!=='2000-01-02T03:04:08.009Z')throw 'snapshot/order';
d.setTime(NaN);log='';var result=d.setSeconds({valueOf:function(){log+='s';d.setTime(17);return 1;}},{valueOf:function(){log+='m';d.setTime(23);return 2;}});
if(!isNaN(result)||d.getTime()!==23||log!=='sm')throw 'invalid callback';
var reads=0,caught=false;try{Date.prototype.setSeconds.call({}, {valueOf:function(){reads++;return 0;}});}catch(e){caught=e.name==='TypeError';}if(!caught||reads)throw 'brand';
d.setTime(42);caught=false;try{d.setSeconds(Symbol());}catch(e){caught=e.name==='TypeError';}if(!caught||d.getTime()!==42)throw 'Symbol';
var marker={};caught=false;try{d.setSeconds(1,{valueOf:function(){d.setTime(77);throw marker;}});}catch(e){caught=e===marker;}if(!caught||d.getTime()!==77)throw 'throw identity';
Object.freeze(d);if(d.setSeconds(2)!==2077||d.getTime()!==2077)throw 'frozen slot';'ok';
"#, &mut host);
        assert_eq!((host.reads,host.writes),(2,2));
    }

    #[test]
    fn dates_local_seconds_use_calendar_inverse_across_dst_fold_and_gap() {
        let mut host = LocalHost { central:true, ..LocalHost::default() };
        local_succeeds(r#"
var d=new Date('2024-11-03T07:30:00.007Z');
if(d.setSeconds(d.getSeconds())!==Date.parse('2024-11-03T06:30:00.007Z'))throw 'fold earlier';
d.setTime(Date.parse('2024-11-03T06:58:30Z'));
if(d.setSeconds(330)!==Date.parse('2024-11-03T08:03:30Z'))throw 'fall calendar rollover';
d.setTime(Date.parse('2024-03-10T07:59:30Z'));
if(d.setSeconds(90)!==Date.parse('2024-03-10T08:00:30Z'))throw 'spring pre-gap offset';'ok';
"#, &mut host);
    }

    #[test]
    fn dates_local_timezone_failure_is_fatal_preserves_mutations_and_recovers() {
        let mut host = LocalHost { fail_inverse:true, ..LocalHost::default() };
        let mut runtime = Runtime::for_test();
        runtime.execute("var d=new Date(42),caught=false;", &mut host).unwrap();
        let error = runtime.execute("try{d.setSeconds({valueOf:function(){d.setTime(17);return 1;}});}catch(e){caught=true;}", &mut host).unwrap_err();
        assert!(error.contains("inverse unavailable"), "{error}");
        runtime.begin_job();
        assert_eq!(runtime.execute("!caught&&d.getTime()===17;", &mut host).unwrap().to_text(),"true");
        host.fail_inverse = false;
        runtime.begin_job();
        assert_eq!(runtime.execute("d.setSeconds(2);", &mut host).unwrap().to_number(),2017.0);
        for value in [f64::NAN,f64::INFINITY,0.5,9_007_199_254_740_992.0] {
            host.bad_local = Some(value);
            runtime.begin_job();
            assert!(runtime.execute("d.getSeconds();", &mut host).unwrap_err().contains("calendar milliseconds"));
        }
        host.bad_local = None;
        for value in [f64::INFINITY,0.5] {
            host.bad_utc = Some(value);
            runtime.begin_job();
            assert!(runtime.execute("d.setSeconds(3);", &mut host).unwrap_err().contains("UTC milliseconds"));
        }
        runtime.begin_job();
        assert_eq!(runtime.execute("d.getTime();", &mut host).unwrap().to_number(),2017.0);
        let reads = host.reads;
        let writes = host.writes;
        runtime.begin_job();
        assert_eq!(runtime.execute("var n=new Date(NaN);isNaN(n.getSeconds())&&isNaN(n.setSeconds(1));", &mut host).unwrap().to_text(),"true");
        assert_eq!((host.reads,host.writes),(reads,writes));
    }

    #[test]
    fn dates_local_string_formats_real_offset_calendar_and_parse_roundtrip() {
        let mut host = LocalHost {offset:19_800_000.0,..LocalHost::default()};
        local_succeeds(r#"
var d=new Date('2024-01-01T20:31:02.999Z');
if(d.toString()!=='Tue Jan 02 2024 02:01:02 GMT+0530'||d.toDateString()!=='Tue Jan 02 2024'||d.toTimeString()!=='02:01:02 GMT+0530'||String(d)!==d.toString())throw 'format';
var called=0,arg={valueOf:function(){called++;throw 'must ignore';},toString:function(){called++;throw 'must ignore';}};
if(Date(arg,Symbol())!=='Thu Jan 01 1970 05:30:00 GMT+0530'||called)throw 'call Date ignores args';
for(var stamp of [0,-1000,Date.parse('0000-01-01T00:00Z'),Date.parse('-000001-01-01T00:00Z'),8640000000000000,-8640000000000000]){
 var v=new Date(stamp);if(Date.parse(v.toString())!==stamp||new Date(v.toString()).getTime()!==stamp)throw 'roundtrip';
}
var bad=new Date(NaN);if(bad.toString()!=='Invalid Date'||bad.toDateString()!=='Invalid Date'||bad.toTimeString()!=='Invalid Date')throw 'invalid';
var n=0;for(var method of ['toString','toDateString','toTimeString'])try{Date.prototype[method].call({});}catch(e){if(e.name==='TypeError')n++;}if(n!==3)throw 'brands';
'ok';
"#,&mut host);
        let reads = host.reads;
        local_succeeds("var n=new Date(NaN);n.toString();n.toDateString();n.toTimeString();'ok';",&mut host);
        assert_eq!(host.reads,reads);
    }

    #[test]
    fn dates_local_strings_distinguish_dst_folds_and_reject_malformed_offsets() {
        let mut host = LocalHost {central:true,..LocalHost::default()};
        local_succeeds(r#"
var a=new Date('2024-11-03T06:30:00Z'),b=new Date('2024-11-03T07:30:00Z');
if(a.toString()!=='Sun Nov 03 2024 01:30:00 GMT-0500'||b.toString()!=='Sun Nov 03 2024 01:30:00 GMT-0600')throw 'fold strings';
if(Date.parse(a.toString())!==a.getTime()||Date.parse(b.toString())!==b.getTime())throw 'fold roundtrip';
for(var text of ['Thu Jan 01 1970 00:00:00 GMT+2400','Thu Jan 01 1970 00:00:00 GMT+0060','Thu Jan 01 1970 00:00:00 GMT?0000','Fri Jan 01 1970 00:00:00 GMT+0000','Thu Jan 32 1970 00:00:00 GMT+0000'])if(!isNaN(Date.parse(text)))throw 'malformed';'ok';
"#,&mut host);
        let mut nepal = LocalHost {offset:20_700_000.0,..LocalHost::default()};
        local_succeeds("if(new Date(0).toTimeString()!=='05:45:00 GMT+0545')throw 'quarter hour';'ok';",&mut nepal);
        let mut west = LocalHost {offset:-21_600_000.0,..LocalHost::default()};
        local_succeeds("var d=new Date(-8640000000000000);if(Date.parse(d.toString())!==d.getTime())throw 'lower boundary offset';'ok';",&mut west);
    }

    #[test]
    fn dates_parser_and_allocations_share_bounds() {
        assert!(parse(&"x".repeat(MAX_DATE_TEXT + 1)).unwrap().is_nan());
        let mut runtime = Runtime::for_test();
        runtime.execute("var caught=false;", &mut NoHost).unwrap();
        let error = runtime
            .execute(
                "try{for(var i=0;i<10000;i++)new Date(i);}catch(e){caught=true;}",
                &mut NoHost,
            )
            .unwrap_err();
        assert!(error.contains("limit"), "{error}");
        runtime.begin_job();
        assert_eq!(
            runtime.execute("caught;", &mut NoHost).unwrap().to_text(),
            "false"
        );
    }
}
