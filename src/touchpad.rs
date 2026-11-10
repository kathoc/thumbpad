//! Descriptor-driven Precision Touchpad input. No fixed report byte offsets.
#![allow(unsafe_op_in_unsafe_fn)]
use std::{
    collections::{HashMap, HashSet},
    mem::{offset_of, size_of},
    time::Instant,
};
use windows::Win32::{Devices::HumanInterfaceDevice::*, UI::Input::*};

#[derive(Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}
struct Contact {
    point: Point,
    accepted: bool,
}
struct Gate {
    contacts: HashMap<u32, Contact>,
    last_motion: Option<Instant>,
    side: f64,
    top: f64,
    grace: std::time::Duration,
}
impl Gate {
    fn new(side: f64, top: f64, ms: u64) -> Self {
        Self {
            contacts: HashMap::new(),
            last_motion: None,
            side: side / 100.0,
            top: top / 100.0,
            grace: std::time::Duration::from_millis(ms),
        }
    }
    fn sample(&mut self, id: u32, point: Point, down: bool, now: Instant) {
        if !down {
            self.contacts.remove(&id);
            return;
        }
        if let Some(c) = self.contacts.get_mut(&id) {
            // Ignore tiny sensor jitter; only accepted gestures can extend the grace.
            if c.accepted
                && ((c.point.x - point.x).abs() > 0.0005 || (c.point.y - point.y).abs() > 0.0005)
            {
                self.last_motion = Some(now);
            }
            c.point = point;
        } else {
            let interior =
                point.x >= self.side && point.x <= 1.0 - self.side && point.y >= self.top;
            let recent = self
                .last_motion
                .is_some_and(|t| now.saturating_duration_since(t) <= self.grace);
            self.contacts.insert(
                id,
                Contact {
                    point,
                    accepted: interior || recent,
                },
            );
        }
    }
    fn touching(&self) -> bool {
        self.contacts.values().any(|c| c.accepted)
    }
}
struct Finger {
    link: u16,
    report_id: u8,
    confidence: bool,
    x: HIDP_VALUE_CAPS,
    y: HIDP_VALUE_CAPS,
}
#[derive(Clone, Copy)]
struct Sample {
    id: u32,
    point: Point,
    down: bool,
}
#[derive(Default)]
struct Frame {
    expected: usize,
    samples: Vec<Sample>,
}
impl Frame {
    fn start(&mut self, count: usize) {
        if count > 0 {
            self.expected = count;
            self.samples.clear();
        }
    }
    fn remaining(&self) -> usize {
        self.expected.saturating_sub(self.samples.len())
    }
    fn commit(&mut self, gate: &mut Gate, now: Instant) {
        if self.remaining() != 0 {
            return;
        }
        // Only a complete frame may remove contacts, including serial/hybrid reports.
        let live: HashSet<_> = self
            .samples
            .iter()
            .filter(|s| s.down)
            .map(|s| s.id)
            .collect();
        gate.contacts.retain(|id, _| live.contains(id));
        for sample in &self.samples {
            gate.sample(sample.id, sample.point, sample.down, now);
        }
        self.expected = 0;
        self.samples.clear();
    }
}
struct Device {
    prep: Vec<usize>,
    fingers: Vec<Finger>,
    gate: Gate,
    name: String,
    reports: u64,
    errors: u64,
    count_link: u16,
    count_report: u8,
    frame: Frame,
}
impl Device {
    unsafe fn open(
        handle: windows::Win32::Foundation::HANDLE,
        side: f64,
        top: f64,
        ms: u64,
    ) -> Option<Self> {
        let mut bytes = 0;
        if GetRawInputDeviceInfoW(Some(handle), RIDI_PREPARSEDDATA, None, &mut bytes) == u32::MAX
            || bytes == 0
        {
            return None;
        }
        let mut prep = vec![0usize; (bytes as usize).div_ceil(size_of::<usize>())];
        if GetRawInputDeviceInfoW(
            Some(handle),
            RIDI_PREPARSEDDATA,
            Some(prep.as_mut_ptr().cast()),
            &mut bytes,
        ) == u32::MAX
        {
            return None;
        }
        let pp = PHIDP_PREPARSED_DATA(prep.as_ptr() as isize);
        let mut caps = HIDP_CAPS::default();
        if HidP_GetCaps(pp, &mut caps) != HIDP_STATUS_SUCCESS
            || caps.UsagePage != 0x0d
            || caps.Usage != 5
        {
            return None;
        }
        let mut buttons = vec![HIDP_BUTTON_CAPS::default(); caps.NumberInputButtonCaps as usize];
        let mut n = buttons.len() as u16;
        if HidP_GetButtonCaps(HidP_Input, buttons.as_mut_ptr(), &mut n, pp) != HIDP_STATUS_SUCCESS {
            return None;
        }
        buttons.truncate(n as usize);
        let mut values = vec![HIDP_VALUE_CAPS::default(); caps.NumberInputValueCaps as usize];
        let mut n = values.len() as u16;
        if HidP_GetValueCaps(HidP_Input, values.as_mut_ptr(), &mut n, pp) != HIDP_STATUS_SUCCESS {
            return None;
        }
        values.truncate(n as usize);
        let count_cap = values
            .iter()
            .find(|v| v.UsagePage == 0xd && !v.IsRange && v.Anonymous.NotRange.Usage == 0x54)?;
        let count_link = count_cap.LinkCollection;
        let count_report = count_cap.ReportID;
        let contains = |b: &HIDP_BUTTON_CAPS, usage| {
            b.UsagePage == 0xd
                && if b.IsRange {
                    usage >= b.Anonymous.Range.UsageMin && usage <= b.Anonymous.Range.UsageMax
                } else {
                    b.Anonymous.NotRange.Usage == usage
                }
        };
        let mut seen = HashSet::new();
        let mut fingers = Vec::new();
        for b in &buttons {
            if !contains(b, 0x42) || !seen.insert((b.LinkCollection, b.ReportID)) {
                continue;
            }
            let axis = |usage| {
                values
                    .iter()
                    .find(|v| {
                        v.UsagePage == 1
                            && v.LinkCollection == b.LinkCollection
                            && v.ReportID == b.ReportID
                            && !v.IsRange
                            && v.Anonymous.NotRange.Usage == usage
                    })
                    .copied()
            };
            if let (Some(x), Some(y)) = (axis(0x30), axis(0x31)) {
                if x.LogicalMax <= x.LogicalMin || y.LogicalMax <= y.LogicalMin {
                    continue;
                }
                fingers.push(Finger {
                    link: b.LinkCollection,
                    report_id: b.ReportID,
                    confidence: buttons.iter().any(|c| {
                        c.LinkCollection == b.LinkCollection
                            && c.ReportID == b.ReportID
                            && contains(c, 0x47)
                    }),
                    x,
                    y,
                });
            }
        }
        let mut count = 0;
        GetRawInputDeviceInfoW(Some(handle), RIDI_DEVICENAME, None, &mut count);
        let mut name = vec![0u16; count as usize + 1];
        GetRawInputDeviceInfoW(
            Some(handle),
            RIDI_DEVICENAME,
            Some(name.as_mut_ptr().cast()),
            &mut count,
        );
        let name = String::from_utf16_lossy(&name[..(count as usize).min(name.len())])
            .trim_end_matches('\0')
            .to_string();
        fingers.sort_by_key(|f| f.link);
        Some(Self {
            prep,
            fingers,
            gate: Gate::new(side, top, ms),
            name,
            reports: 0,
            errors: 0,
            count_link,
            count_report,
            frame: Frame::default(),
        })
    }
    unsafe fn report(&mut self, report: &mut [u8]) {
        if report.is_empty() {
            return;
        }
        let pp = PHIDP_PREPARSED_DATA(self.prep.as_ptr() as isize);
        self.reports += 1;
        if self.count_report != 0 && report[0] != self.count_report {
            return;
        }
        let mut total = 0;
        if HidP_GetUsageValue(
            HidP_Input,
            0xd,
            Some(self.count_link),
            0x54,
            &mut total,
            pp,
            report,
        ) != HIDP_STATUS_SUCCESS
            || total > 100
        {
            self.errors += 1;
            self.gate.contacts.clear();
            self.frame = Frame::default();
            return;
        }
        self.frame.start(total as usize);
        let slots = self.frame.remaining();
        for finger in &self.fingers {
            if self.frame.samples.len() >= self.frame.expected {
                break;
            }
            if finger.report_id != 0 && finger.report_id != report[0] {
                continue;
            }
            let mut usages = [0u16; 64];
            let mut count = usages.len() as u32;
            if HidP_GetUsages(
                HidP_Input,
                0xd,
                Some(finger.link),
                usages.as_mut_ptr(),
                &mut count,
                pp,
                report,
            ) != HIDP_STATUS_SUCCESS
            {
                self.errors += 1;
                self.gate.contacts.clear();
                self.frame = Frame::default();
                return;
            }
            let usages = &usages[..(count as usize).min(usages.len())];
            let down = usages.contains(&0x42) && (!finger.confidence || usages.contains(&0x47));
            let mut id = 0;
            if HidP_GetUsageValue(
                HidP_Input,
                0xd,
                Some(finger.link),
                0x51,
                &mut id,
                pp,
                report,
            ) != HIDP_STATUS_SUCCESS
            {
                self.errors += 1;
                self.gate.contacts.clear();
                self.frame = Frame::default();
                return;
            }
            let mut x = 0;
            let mut y = 0;
            if down
                && (HidP_GetUsageValue(HidP_Input, 1, Some(finger.link), 0x30, &mut x, pp, report)
                    != HIDP_STATUS_SUCCESS
                    || HidP_GetUsageValue(
                        HidP_Input,
                        1,
                        Some(finger.link),
                        0x31,
                        &mut y,
                        pp,
                        report,
                    ) != HIDP_STATUS_SUCCESS)
            {
                self.errors += 1;
                self.gate.contacts.clear();
                self.frame = Frame::default();
                return;
            }
            let norm = |v: u32, cap: &HIDP_VALUE_CAPS| {
                ((v as f64 - cap.LogicalMin as f64)
                    / (cap.LogicalMax as f64 - cap.LogicalMin as f64))
                    .clamp(0.0, 1.0)
            };
            self.frame.samples.push(Sample {
                id,
                point: Point {
                    x: norm(x, &finger.x),
                    y: norm(y, &finger.y),
                },
                down,
            });
        }
        if slots > 0 && self.frame.samples.is_empty() {
            self.errors += 1;
        }
        self.frame.commit(&mut self.gate, Instant::now());
    }
}
pub struct Devices {
    devices: HashMap<usize, Option<Device>>,
    side: f64,
    top: f64,
    ms: u64,
}
impl Devices {
    pub fn new(side: f64, top: f64, ms: u64) -> Self {
        let mut result = Self {
            devices: HashMap::new(),
            side,
            top,
            ms,
        };
        unsafe {
            let mut count = 0;
            let size = size_of::<RAWINPUTDEVICELIST>() as u32;
            if GetRawInputDeviceList(None, &mut count, size) != u32::MAX {
                let mut list = vec![RAWINPUTDEVICELIST::default(); count as usize];
                let read = GetRawInputDeviceList(Some(list.as_mut_ptr()), &mut count, size);
                if read != u32::MAX {
                    for item in list
                        .iter()
                        .take(read as usize)
                        .filter(|d| d.dwType == RIM_TYPEHID)
                    {
                        if let Some(device) = Device::open(item.hDevice, side, top, ms) {
                            result.devices.insert(item.hDevice.0 as usize, Some(device));
                        }
                    }
                }
            }
        }
        result
    }
    pub fn clear(&mut self) {
        self.devices.clear();
    }
    pub fn touching(&self) -> bool {
        self.devices.values().flatten().any(|d| d.gate.touching())
    }
    pub fn block_pointer(&self) -> bool {
        !self.touching()
            && self
                .devices
                .values()
                .flatten()
                .any(|d| !d.gate.contacts.is_empty())
    }
    pub unsafe fn input(&mut self, handle: HRAWINPUT) {
        let mut bytes = 0;
        let header = size_of::<RAWINPUTHEADER>() as u32;
        if GetRawInputData(handle, RID_INPUT, None, &mut bytes, header) == u32::MAX
            || bytes < (offset_of!(RAWINPUT, data) + offset_of!(RAWHID, bRawData)) as u32
        {
            return;
        }
        let mut buffer = vec![0usize; (bytes as usize).div_ceil(size_of::<usize>())];
        if GetRawInputData(
            handle,
            RID_INPUT,
            Some(buffer.as_mut_ptr().cast()),
            &mut bytes,
            header,
        ) == u32::MAX
        {
            return;
        }
        let raw = &*(buffer.as_ptr() as *const RAWINPUT);
        if raw.header.dwType != RIM_TYPEHID.0 {
            return;
        }
        let key = raw.header.hDevice.0 as usize;
        let entry = self
            .devices
            .entry(key)
            .or_insert_with(|| Device::open(raw.header.hDevice, self.side, self.top, self.ms));
        let Some(device) = entry else {
            return;
        };
        let size = raw.data.hid.dwSizeHid as usize;
        let count = raw.data.hid.dwCount as usize;
        let offset = offset_of!(RAWINPUT, data) + offset_of!(RAWHID, bRawData);
        if size == 0
            || size
                .checked_mul(count)
                .and_then(|n| n.checked_add(offset))
                .is_none_or(|n| n > bytes as usize)
        {
            return;
        }
        let reports = std::slice::from_raw_parts_mut(
            (buffer.as_mut_ptr() as *mut u8).add(offset),
            size * count,
        );
        for report in reports.chunks_exact_mut(size) {
            device.report(report);
        }
    }
    pub fn summary(&self) -> String {
        let devices: Vec<_> = self.devices.values().flatten().collect();
        let supported = devices.iter().filter(|d| !d.fingers.is_empty()).count();
        let reports: u64 = devices.iter().map(|d| d.reports).sum();
        let errors: u64 = devices.iter().map(|d| d.errors).sum();
        let name = devices
            .first()
            .map(|d| d.name.chars().take(88).collect::<String>())
            .unwrap_or_else(|| {
                "Waiting for touchpad reports. Move a finger on the touchpad.".into()
            });
        format!(
            "Decoded devices: {supported}   HID reports: {reports}   Parse errors: {errors}\r\nClick mode: {}\r\nStart exclusions: left/right {}%, top {}% / grace {} ms\r\n{name}",
            if self.touching() {
                "ACTIVE"
            } else {
                "INACTIVE"
            },
            self.side,
            self.top,
            self.ms
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn p(x: f64, y: f64) -> Point {
        Point { x, y }
    }
    #[test]
    fn hybrid_frame_preserves_contacts_until_all_packets_arrive() {
        let mut g = Gate::new(12.0, 15.0, 500);
        let t = Instant::now();
        let mut frame = Frame::default();
        frame.start(2);
        frame.samples.push(Sample {
            id: 0,
            point: p(0.5, 0.5),
            down: true,
        });
        frame.commit(&mut g, t);
        assert!(!g.touching());
        frame.start(0); // Continuation packet, NOT an empty frame.
        frame.samples.push(Sample {
            id: 1,
            point: p(0.99, 0.5),
            down: true,
        });
        frame.commit(&mut g, t);
        assert!(g.touching());
        assert_eq!(g.contacts.len(), 2);
        frame.start(1);
        frame.samples.push(Sample {
            id: 0,
            point: p(0.5, 0.5),
            down: false,
        });
        frame.commit(&mut g, t);
        assert!(!g.touching());
        assert!(g.contacts.is_empty());
    }
    #[test]
    fn zero_contact_frame_clears_state() {
        let mut g = Gate::new(12.0, 15.0, 500);
        let t = Instant::now();
        g.sample(1, p(0.5, 0.5), true, t);
        let mut frame = Frame::default();
        frame.start(0);
        frame.commit(&mut g, t);
        assert!(!g.touching());
        assert!(g.contacts.is_empty());
    }
    #[test]
    fn blocked_start_stays_blocked_until_lift() {
        let mut g = Gate::new(12.0, 15.0, 500);
        let t = Instant::now();
        g.sample(1, p(0.02, 0.5), true, t);
        assert!(!g.touching());
        g.sample(1, p(0.5, 0.5), true, t);
        assert!(!g.touching());
        g.sample(1, p(0.5, 0.5), false, t);
        g.sample(1, p(0.5, 0.5), true, t);
        assert!(g.touching());
    }
    #[test]
    fn interior_start_can_reach_edges_and_reconnect_for_half_second() {
        let mut g = Gate::new(12.0, 15.0, 500);
        let t = Instant::now();
        g.sample(1, p(0.5, 0.5), true, t);
        g.sample(1, p(0.99, 0.01), true, t);
        assert!(g.touching());
        g.sample(1, p(0.99, 0.01), false, t);
        assert!(!g.touching());
        g.sample(2, p(0.99, 0.01), true, t + Duration::from_millis(499));
        assert!(g.touching());
        g.sample(2, p(0.99, 0.01), false, t);
        g.sample(3, p(0.99, 0.01), true, t + Duration::from_millis(501));
        assert!(!g.touching());
    }
    #[test]
    fn top_blocked_bottom_allowed_and_blocked_motion_cannot_create_grace() {
        let mut g = Gate::new(12.0, 15.0, 500);
        let t = Instant::now();
        g.sample(1, p(0.5, 0.01), true, t);
        assert!(!g.touching());
        g.sample(1, p(0.6, 0.01), true, t);
        g.sample(1, p(0.6, 0.01), false, t);
        g.sample(2, p(0.99, 0.5), true, t);
        assert!(!g.touching());
        g.sample(3, p(0.5, 0.99), true, t);
        assert!(g.touching());
    }
}
