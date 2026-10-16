//! Battery telemetry via IOKit's `AppleSmartBattery` registry service.
//!
//! Talks to IOKit directly — no subprocesses, no parsing of `ioreg` text.
//! All CoreFoundation / IOKit calls are declared here as a thin FFI layer.

use std::ffi::{CString, c_char, c_void};
use std::ptr;

type CFRef = *const c_void;
type IoObject = u32;

const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const K_CF_NUMBER_SINT64: i32 = 4;
/// Sentinel used by SMC for "unknown" time values (u16::MAX minutes).
const TIME_UNKNOWN: i64 = 65535;

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringCreateWithCString(alloc: CFRef, s: *const c_char, enc: u32) -> CFRef;
    fn CFRelease(cf: CFRef);
    fn CFGetTypeID(cf: CFRef) -> usize;
    fn CFNumberGetTypeID() -> usize;
    fn CFBooleanGetTypeID() -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFDictionaryGetTypeID() -> usize;
    fn CFDictionaryGetValue(d: CFRef, key: CFRef) -> CFRef;
    fn CFNumberGetValue(n: CFRef, t: i32, out: *mut c_void) -> u8;
    fn CFBooleanGetValue(b: CFRef) -> u8;
    fn CFStringGetCStringPtr(s: CFRef, enc: u32) -> *const c_char;
    fn CFStringGetCString(s: CFRef, buf: *mut c_char, len: isize, enc: u32) -> u8;
}

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    fn IOServiceMatching(name: *const c_char) -> CFRef;
    fn IOServiceGetMatchingService(main_port: u32, matching: CFRef) -> IoObject;
    fn IORegistryEntryCreateCFProperties(
        entry: IoObject,
        props: *mut CFRef,
        alloc: CFRef,
        opts: u32,
    ) -> i32;
    fn IOObjectRelease(obj: IoObject) -> i32;
}

// ---------------------------------------------------------------------------
// Minimal CF helpers
// ---------------------------------------------------------------------------

fn cf_string(s: &str) -> CFRef {
    let c = CString::new(s).unwrap_or_default();
    unsafe { CFStringCreateWithCString(ptr::null(), c.as_ptr(), K_CF_STRING_ENCODING_UTF8) }
}

fn dict_get(dict: CFRef, key: &str) -> CFRef {
    if dict.is_null() {
        return ptr::null();
    }
    let k = cf_string(key);
    if k.is_null() {
        return ptr::null();
    }
    let v = unsafe { CFDictionaryGetValue(dict, k) };
    unsafe { CFRelease(k) };
    v
}

fn type_id(v: CFRef) -> usize {
    unsafe { CFGetTypeID(v) }
}

/// Extract an integer from a non-null CoreFoundation number-ish value.
///
/// `CFNumberGetValue` converts between number types itself, so requesting
/// `SInt64` also covers float-backed numbers.
fn number_as_i64(v: CFRef) -> Option<i64> {
    unsafe {
        if type_id(v) == CFNumberGetTypeID() {
            let mut out: i64 = 0;
            if CFNumberGetValue(v, K_CF_NUMBER_SINT64, &mut out as *mut i64 as *mut c_void) != 0 {
                return Some(out);
            }
        } else if type_id(v) == CFBooleanGetTypeID() {
            return Some(CFBooleanGetValue(v) as i64);
        }
    }
    None
}

fn get_i64(dict: CFRef, key: &str) -> Option<i64> {
    let v = dict_get(dict, key);
    if v.is_null() {
        return None;
    }
    number_as_i64(v)
}

fn get_bool(dict: CFRef, key: &str) -> Option<bool> {
    let v = dict_get(dict, key);
    if v.is_null() {
        return None;
    }
    unsafe {
        if type_id(v) == CFBooleanGetTypeID() {
            return Some(CFBooleanGetValue(v) != 0);
        }
    }
    number_as_i64(v).map(|n| n != 0)
}

fn get_str(dict: CFRef, key: &str) -> Option<String> {
    let v = dict_get(dict, key);
    if v.is_null() || type_id(v) != unsafe { CFStringGetTypeID() } {
        return None;
    }
    unsafe {
        let p = CFStringGetCStringPtr(v, K_CF_STRING_ENCODING_UTF8);
        if !p.is_null() {
            return Some(std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned());
        }
        let mut buf = vec![0u8; 512];
        let ok = CFStringGetCString(
            v,
            buf.as_mut_ptr() as *mut c_char,
            buf.len() as isize,
            K_CF_STRING_ENCODING_UTF8,
        );
        if ok != 0 {
            let len = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
            return Some(String::from_utf8_lossy(&buf[..len]).into_owned());
        }
    }
    None
}

fn get_dict(dict: CFRef, key: &str) -> Option<CFRef> {
    let v = dict_get(dict, key);
    if v.is_null() || type_id(v) != unsafe { CFDictionaryGetTypeID() } {
        return None;
    }
    Some(v)
}

// ---------------------------------------------------------------------------
// Public model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Condition {
    Normal,
    ServiceRecommended,
}

impl Condition {
    pub fn as_str(self) -> &'static str {
        match self {
            Condition::Normal => "Normal",
            Condition::ServiceRecommended => "Service Recommended",
        }
    }
}

/// Classify the battery condition.
///
/// `Service Recommended` requires either a reported permanent failure or a
/// *measurable* health below 80%. When the design capacity is unknown the
/// health percentage is 0.0, so the health heuristic is skipped rather than
/// misreporting an unreadable battery as failed.
fn condition_from(permanent_failure: bool, design_capacity: u32, health_percent: f64) -> Condition {
    if permanent_failure || (design_capacity > 0 && health_percent < 80.0) {
        Condition::ServiceRecommended
    } else {
        Condition::Normal
    }
}

#[derive(Debug, Clone)]
pub struct BatteryInfo {
    /// False on desktops / VMs with no battery.
    pub present: bool,
    pub charge_percent: f64,
    pub is_charging: bool,
    pub fully_charged: bool,
    pub on_ac: bool,
    /// Minutes until empty (discharging).
    pub time_to_empty_min: Option<u32>,
    /// Minutes until full (charging).
    pub time_to_full_min: Option<u32>,
    pub cycle_count: u32,
    pub max_cycle_count: u32,
    pub design_capacity: u32,
    pub full_charge_capacity: u32,
    pub current_capacity: u32,
    pub health_percent: f64,
    pub voltage_mv: u32,
    /// Signed: positive while charging, negative while discharging.
    pub amperage_ma: i32,
    pub watts: f64,
    pub temperature_c: f64,
    pub condition: Condition,
    pub adapter_watts: Option<u32>,
    pub manufacturer: Option<String>,
    pub device_name: Option<String>,
    pub serial: Option<String>,
    pub manufacture_date: Option<String>,
}

impl BatteryInfo {
    pub fn absent() -> Self {
        BatteryInfo {
            present: false,
            charge_percent: 0.0,
            is_charging: false,
            fully_charged: false,
            on_ac: false,
            time_to_empty_min: None,
            time_to_full_min: None,
            cycle_count: 0,
            max_cycle_count: 0,
            design_capacity: 0,
            full_charge_capacity: 0,
            current_capacity: 0,
            health_percent: 0.0,
            voltage_mv: 0,
            amperage_ma: 0,
            watts: 0.0,
            temperature_c: 0.0,
            condition: Condition::Normal,
            adapter_watts: None,
            manufacturer: None,
            device_name: None,
            serial: None,
            manufacture_date: None,
        }
    }

    /// Runtime state: charging / on battery / full.
    pub fn state(&self) -> &'static str {
        if self.fully_charged {
            "Fully Charged"
        } else if self.is_charging {
            "Charging"
        } else if self.on_ac {
            "On AC Power"
        } else {
            "On Battery"
        }
    }
}

// ---------------------------------------------------------------------------
// Read
// ---------------------------------------------------------------------------

/// Read the current battery state. Never fails: returns `present: false`
/// when no battery service exists (desktop Macs, VMs).
pub fn read() -> BatteryInfo {
    unsafe {
        let matching = IOServiceMatching(c"AppleSmartBattery".as_ptr());
        if matching.is_null() {
            return BatteryInfo::absent();
        }
        // IOServiceGetMatchingService consumes the matching dictionary.
        let service = IOServiceGetMatchingService(0, matching);
        if service == 0 {
            return BatteryInfo::absent();
        }
        let mut props: CFRef = ptr::null();
        let kr = IORegistryEntryCreateCFProperties(service, &mut props, ptr::null(), 0);
        IOObjectRelease(service);
        if kr != 0 || props.is_null() {
            return BatteryInfo::absent();
        }

        let info = parse(props);
        CFRelease(props);
        info
    }
}

fn parse(props: CFRef) -> BatteryInfo {
    let battery_data = get_dict(props, "BatteryData").unwrap_or(ptr::null());
    let adapter = get_dict(props, "AdapterDetails").unwrap_or(ptr::null());

    let design_capacity = get_i64(props, "DesignCapacity")
        .or(get_i64(battery_data, "DesignCapacity"))
        .unwrap_or(0) as u32;

    // Prefer the mAh keys. Newer gauges (e.g. bq40z651) drop the top-level
    // AppleRaw* keys and keep the mAh values inside BatteryData instead; the
    // bare top-level MaxCapacity key is a last resort because on many models
    // it holds a 0-100 percentage that must be scaled, never read as mAh.
    let full_charge_capacity = get_i64(props, "AppleRawMaxCapacity")
        .or(get_i64(battery_data, "AppleRawMaxCapacity"))
        .or(get_i64(props, "NominalChargeCapacity"))
        .or(get_i64(battery_data, "NominalChargeCapacity"))
        .or(get_i64(battery_data, "FullChargeCapacity"))
        .map(|v| v.max(0) as u32)
        .or_else(|| get_i64(props, "MaxCapacity").map(|v| capacity_from_scale(v, design_capacity)))
        .unwrap_or(0);

    // Raw mAh when available. The bare CurrentCapacity key is never used as
    // mAh because it holds a percentage on every known model.
    let current_capacity_hint = get_i64(props, "AppleRawCurrentCapacity")
        .or(get_i64(battery_data, "AppleRawCurrentCapacity"))
        .or(get_i64(battery_data, "RemainingCapacity"))
        .map(|v| v.max(0) as u32);

    // Charge %: the fuel-gauge UI value is what macOS shows; fall back to ratio.
    let charge_percent = get_i64(battery_data, "UISoc")
        .or(get_i64(battery_data, "StateOfCharge"))
        .map(|v| v as f64)
        .unwrap_or_else(|| {
            if full_charge_capacity > 0 {
                current_capacity_hint.unwrap_or(0) as f64 / full_charge_capacity as f64 * 100.0
            } else {
                0.0
            }
        })
        .clamp(0.0, 100.0);

    let current_capacity = current_capacity_hint
        .unwrap_or_else(|| (charge_percent / 100.0 * full_charge_capacity as f64) as u32);

    let health_percent = if design_capacity > 0 {
        full_charge_capacity as f64 / design_capacity as f64 * 100.0
    } else {
        0.0
    };

    let voltage_mv = get_i64(props, "Voltage").unwrap_or(0) as u32;
    let amperage_ma = get_i64(props, "Amperage")
        .or(get_i64(props, "InstantAmperage"))
        .unwrap_or(0) as i32;
    let watts = voltage_mv as f64 * (amperage_ma as f64).abs() / 1_000_000.0;

    let is_charging = get_bool(props, "IsCharging").unwrap_or(false);
    let fully_charged = get_bool(props, "FullyCharged").unwrap_or(false);
    let on_ac = get_bool(props, "ExternalConnected")
        .or(get_bool(props, "ExternalChargeCapable"))
        .unwrap_or(false);

    // Time values: u16::MAX (65535) is the "unknown" sentinel.
    let clean_time = |v: Option<i64>| -> Option<u32> {
        match v {
            Some(t) if t != TIME_UNKNOWN && t > 0 && t < TIME_UNKNOWN => Some(t as u32),
            _ => None,
        }
    };
    let time_to_empty_min = if is_charging || on_ac {
        // Never show a drain estimate while external power is connected.
        None
    } else {
        clean_time(get_i64(props, "TimeRemaining")).or(clean_time(get_i64(props, "AvgTimeToEmpty")))
    };
    let time_to_full_min = if is_charging {
        clean_time(get_i64(props, "AvgTimeToFull")).or(clean_time(get_i64(props, "TimeRemaining")))
    } else {
        None
    };

    let cycle_count = get_i64(props, "CycleCount")
        .or(get_i64(battery_data, "CycleCount"))
        .unwrap_or(0) as u32;
    let max_cycle_count = get_i64(props, "DesignCycleCount9C").unwrap_or(1000) as u32;

    let condition = condition_from(
        get_i64(props, "PermanentFailureStatus").unwrap_or(0) != 0,
        design_capacity,
        health_percent,
    );

    let manufacture_date = get_i64(battery_data, "ManufactureDate")
        .or(get_i64(props, "ManufactureDate"))
        .and_then(decode_date);

    BatteryInfo {
        present: true,
        charge_percent,
        is_charging,
        fully_charged,
        on_ac,
        time_to_empty_min,
        time_to_full_min,
        cycle_count,
        max_cycle_count,
        design_capacity,
        full_charge_capacity,
        current_capacity,
        health_percent,
        voltage_mv,
        amperage_ma,
        watts,
        temperature_c: get_i64(props, "Temperature")
            .or(get_i64(props, "VirtualTemperature"))
            .or(get_i64(battery_data, "Temperature"))
            .unwrap_or(0) as f64
            / 100.0,
        condition,
        adapter_watts: get_i64(adapter, "Watts").map(|w| w as u32),
        manufacturer: get_str(props, "Manufacturer")
            .or_else(|| get_str(battery_data, "Manufacturer")),
        device_name: get_str(props, "DeviceName"),
        serial: get_str(props, "Serial"),
        manufacture_date,
    }
}

/// Interpret a bare capacity value that may hold either mAh or a 0-100
/// percentage.
///
/// Values above 100 can only be mAh. On many models the bare top-level
/// `MaxCapacity` key holds a percentage of the design capacity instead, so
/// smaller values are treated as such. Returns 0 when the value is
/// non-positive or the reference capacity is unknown.
fn capacity_from_scale(value: i64, reference_mah: u32) -> u32 {
    if value > 100 {
        value as u32
    } else if value > 0 && reference_mah > 0 {
        (reference_mah as i64 * value / 100) as u32
    } else {
        0
    }
}

/// Decode the gauge's manufacture date code.
///
/// The fuel gauge stores a 6-digit date code string inside a u64. Because the
/// buffer is written little-endian, the real code is the LE byte order (e.g.
/// the integer whose big-endian digits read "324082" is the code "280423").
/// Layouts are tried in order of plausibility and only fully valid calendar
/// dates are accepted: DDMMYY, YYMMDD, MMDDYY.
fn decode_date(raw: i64) -> Option<String> {
    let digits = |bytes: [u8; 8]| -> String {
        bytes
            .iter()
            .filter(|b| b.is_ascii_digit())
            .map(|b| *b as char)
            .collect()
    };
    let codes = [digits(raw.to_le_bytes()), digits(raw.to_be_bytes())];

    for code in codes {
        if code.len() != 6 {
            continue;
        }
        let n = |a: usize| code[a..a + 2].parse::<u32>();
        let (Ok(a), Ok(b), Ok(c)) = (n(0), n(2), n(4)) else {
            continue;
        };
        // Each candidate is a (year, month, day) interpretation.
        let candidates = [
            (2000 + c as i32, b, a), // DDMMYY
            (2000 + a as i32, b, c), // YYMMDD
            (2000 + c as i32, a, b), // MMDDYY
        ];
        for (year, month, day) in candidates {
            if is_valid_date(year, month, day) {
                return Some(format!("{year:04}-{month:02}-{day:02}"));
            }
        }
    }
    None
}

/// True for a real Gregorian calendar date with a plausible gauge year.
fn is_valid_date(year: i32, month: u32, day: u32) -> bool {
    if !(2005..2100).contains(&year) || !(1..=12).contains(&month) {
        return false;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if leap {
                29
            } else {
                28
            }
        }
    };
    (1..=days_in_month).contains(&day)
}

// ---------------------------------------------------------------------------
// Formatting helpers (shared by the TUI and the one-shot report)
// ---------------------------------------------------------------------------

pub fn fmt_duration(min: u32) -> String {
    let (h, m) = (min / 60, min % 60);
    match (h, m) {
        (0, _) => format!("{m} min"),
        (_, 0) => format!("{h} h"),
        _ => format!("{h} h {m:02} min"),
    }
}

pub fn fmt_amperage(ma: i32) -> String {
    if ma.abs() >= 1000 {
        format!("{:+.2} A", ma as f64 / 1000.0)
    } else {
        format!("{ma:+} mA")
    }
}

#[cfg(test)]
mod tests;
