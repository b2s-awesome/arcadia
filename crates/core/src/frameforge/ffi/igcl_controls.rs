//! Intel driver controls through IGCL.
//!
//! Two documented groups are used:
//! * **3D features** (`ctlGetSupported3DCapabilities`, `ctlGetSet3DFeature`):
//!   the driver lists which features this adapter has, their value type and
//!   their allowed values; Arcadia only offers what that list contains.
//!   Features that take a structured ("custom") value are listed as not
//!   editable.
//! * **Power and temperature limits** (`ctlOverclockGetProperties`,
//!   `ctlOverclock{Power,Temperature}Limit{Get,Set}V2`): shown at Expert
//!   level, inside the range and in the unit the driver reports. Frequency
//!   and voltage offsets are deliberately not exposed.
//!
//! Settings are written globally (no application name), which is what Intel
//! Graphics Software's "Global settings" page does.
//!
//! The mapping from IGCL structures to [`GpuControl`] is plain code tested on
//! every platform. The calls into `ControlLib.dll` only run on Windows with
//! an Intel driver and have not been exercised on hardware by the automated tests.

use super::igcl_types::*;
use crate::frameforge::gpu::{ControlKind, ControlOption, GpuControl, Risk, Vendor};

/// What Arcadia knows about one IGCL 3D feature (names from `igcl_api.h`).
pub(crate) struct Feature {
    pub id: ctl_3d_feature_t,
    pub key: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// Enum value → label. For flag-style enums the value is the flag itself.
    pub options: &'static [(u32, &'static str)],
    /// The driver's support mask holds the values themselves (flags) rather than `1 << value`.
    pub flags: bool,
}

pub(crate) const FEATURES: &[Feature] = &[
    Feature {
        id: CTL_3D_FEATURE_FRAME_PACING,
        key: "frame_pacing",
        name: "Frame pacing",
        description: "How the driver spaces frames when the game runs faster than the display.",
        options: &[(0, "Off"), (1, "On, no smoothing"), (2, "On, maximum smoothing"), (3, "Competitive gaming")],
        flags: false,
    },
    Feature { id: CTL_3D_FEATURE_FRAME_LIMIT, key: "frame_limit", name: "Frame limit", description: "Caps the frame rate in the driver.", options: &[], flags: false },
    Feature {
        id: CTL_3D_FEATURE_ANISOTROPIC,
        key: "anisotropic",
        name: "Anisotropic filtering",
        description: "Overrides the game's anisotropic filtering level.",
        options: &[(0, "Application choice"), (2, "2x"), (4, "4x"), (8, "8x"), (16, "16x")],
        flags: false,
    },
    Feature { id: CTL_3D_FEATURE_CMAA, key: "cmaa", name: "Conservative morphological anti-aliasing", description: "Driver post-process anti-aliasing.", options: &[(0, "Off"), (1, "Override MSAA"), (2, "Enhance application")], flags: false },
    Feature {
        id: CTL_3D_FEATURE_TEXTURE_FILTERING_QUALITY,
        key: "texture_filtering",
        name: "Texture filtering quality",
        description: "Trades texture filtering quality for speed.",
        options: &[(0, "Performance"), (1, "Balanced"), (2, "Quality")],
        flags: false,
    },
    Feature { id: CTL_3D_FEATURE_ADAPTIVE_TESSELLATION, key: "adaptive_tessellation", name: "Adaptive tessellation", description: "Lets the driver lower tessellation detail where it is not visible.", options: &[(0, "Off"), (1, "On")], flags: false },
    Feature { id: CTL_3D_FEATURE_SHARPENING_FILTER, key: "sharpening", name: "Sharpening filter", description: "Driver image sharpening strength.", options: &[], flags: false },
    Feature {
        id: CTL_3D_FEATURE_GAMING_FLIP_MODES,
        key: "flip_mode",
        name: "Vertical sync mode",
        description: "How frames are presented: the game's own choice, V-Sync on or off, Smooth Sync, Speed Frame or a capped frame rate.",
        options: &[(1, "Application default"), (2, "V-Sync off"), (4, "V-Sync on"), (8, "Smooth Sync"), (16, "Speed Frame"), (32, "Capped FPS"), (64, "V-Sync off (ignore allow list)")],
        flags: true,
    },
    Feature { id: CTL_3D_FEATURE_LOW_LATENCY, key: "low_latency", name: "Low latency mode", description: "Reduces the render queue to cut input delay.", options: &[(0, "Off"), (1, "On"), (2, "On with boost")], flags: false },
    Feature { id: CTL_3D_FEATURE_VRR_WINDOWED_BLT, key: "vrr_windowed", name: "Variable refresh in windowed games", description: "Variable refresh rate for games that present in a window.", options: &[(0, "Auto"), (1, "On"), (2, "Off")], flags: false },
];

/// Features the driver may list that Arcadia shows but does not edit.
const READ_ONLY_FEATURES: &[(ctl_3d_feature_t, &str, &str)] = &[
    (CTL_3D_FEATURE_ENDURANCE_GAMING, "Endurance Gaming", "It takes a structured value; change it in Intel Graphics Software."),
    (CTL_3D_FEATURE_ADAPTIVE_SYNC_PLUS, "Adaptive Sync Plus", "It takes a structured value; change it in Intel Graphics Software."),
    (CTL_3D_FEATURE_FRAME_GENERATION, "Frame generation override", "It takes a structured value; change it in Intel Graphics Software."),
    (CTL_3D_FEATURE_MSAA, "MSAA override", "Arcadia does not edit this setting."),
];

#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn feature_by_key(key: &str) -> Option<&'static Feature> {
    FEATURES.iter().find(|f| f.key == key)
}

pub(crate) fn control_id(key: &str) -> String {
    format!("igcl.3d.{key}")
}

/// Build the control for one feature the driver listed. `current` is the value read back (if it could be read).
pub(crate) fn control_from_details(d: &ctl_3d_feature_details_t, current: Option<f64>) -> Option<GpuControl> {
    let base = |id: String, name: &str, description: &str| GpuControl {
        id,
        vendor: Vendor::Intel,
        name: name.to_string(),
        description: description.to_string(),
        category: "3D settings".into(),
        kind: ControlKind::ReadOnly,
        value: current,
        default: None,
        supported: true,
        unsupported_reason: None,
        risk: Risk::Low,
        level: 2,
        requires_restart: (d.FeatureMiscSupport as i32 & CTL_3D_FEATURE_MISC_FLAG_LIVE_CHANGE) == 0,
    };
    if let Some((_, name, why)) = READ_ONLY_FEATURES.iter().find(|x| x.0 == d.FeatureType) {
        let mut c = base(format!("igcl.3d.feature{}", d.FeatureType), name, "");
        c.supported = false;
        c.value = None;
        c.unsupported_reason = Some((*why).to_string());
        return Some(c);
    }
    let f = FEATURES.iter().find(|f| f.id == d.FeatureType)?;
    let mut c = base(control_id(f.key), f.name, f.description);
    // SAFETY: the union member read is the one named by `ValueType`.
    unsafe {
        match d.ValueType {
            CTL_PROPERTY_VALUE_TYPE_BOOL => {
                c.kind = ControlKind::Toggle;
                c.default = Some(if d.Value.BoolType.DefaultState { 1.0 } else { 0.0 });
            }
            CTL_PROPERTY_VALUE_TYPE_ENUM => {
                let mask = d.Value.EnumType.SupportedTypes;
                let options: Vec<ControlOption> = f
                    .options
                    .iter()
                    .filter(|(v, _)| if f.flags { mask & (*v as u64) != 0 } else { *v < 64 && mask & (1u64 << *v) != 0 })
                    .map(|(v, label)| ControlOption { value: *v as f64, label: (*label).to_string() })
                    .collect();
                if options.is_empty() {
                    c.supported = false;
                    c.unsupported_reason = Some("The driver lists this feature but none of the values Arcadia knows.".into());
                }
                c.kind = ControlKind::Choice { options };
                c.default = Some(d.Value.EnumType.DefaultType as f64);
            }
            CTL_PROPERTY_VALUE_TYPE_FLOAT => {
                let r = d.Value.FloatType.RangeInfo;
                c.kind = range(r.min_possible_value as f64, r.max_possible_value as f64, r.step_size as f64, f)?;
                c.default = Some(r.default_value as f64);
            }
            CTL_PROPERTY_VALUE_TYPE_INT32 => {
                let r = d.Value.IntType.RangeInfo;
                c.kind = range(r.min_possible_value as f64, r.max_possible_value as f64, r.step_size as f64, f)?;
                c.default = Some(r.default_value as f64);
            }
            CTL_PROPERTY_VALUE_TYPE_UINT32 => {
                let r = d.Value.UIntType.RangeInfo;
                c.kind = range(r.min_possible_value as f64, r.max_possible_value as f64, r.step_size as f64, f)?;
                c.default = Some(r.default_value as f64);
            }
            _ => {
                c.supported = false;
                c.value = None;
                c.unsupported_reason = Some("On this driver the feature takes a structured value that Arcadia does not edit.".into());
            }
        }
    }
    Some(c)
}

fn range(min: f64, max: f64, step: f64, f: &Feature) -> Option<ControlKind> {
    if !(min.is_finite() && max.is_finite()) || max < min {
        return None;
    }
    let unit = if f.id == CTL_3D_FEATURE_FRAME_LIMIT { "FPS" } else { "" };
    Some(ControlKind::Range { min, max, step: if step.is_finite() && step > 0.0 { step } else { 1.0 }, unit: unit.into() })
}

/// Read the current value out of a get/set block as a number.
pub(crate) fn value_of(g: &ctl_3d_feature_getset_t) -> Option<f64> {
    // SAFETY: the union member read is the one named by `ValueType`.
    unsafe {
        Some(match g.ValueType {
            CTL_PROPERTY_VALUE_TYPE_BOOL => g.Value.BoolType.Enable as u8 as f64,
            CTL_PROPERTY_VALUE_TYPE_ENUM => g.Value.EnumType.EnableType as f64,
            CTL_PROPERTY_VALUE_TYPE_FLOAT => g.Value.FloatType.Value as f64,
            CTL_PROPERTY_VALUE_TYPE_INT32 => g.Value.IntType.Value as f64,
            CTL_PROPERTY_VALUE_TYPE_UINT32 => g.Value.UIntType.Value as f64,
            _ => return None,
        })
    }
}

/// A zeroed, sized get/set block for a global (not per-application) feature.
pub(crate) fn getset(feature: ctl_3d_feature_t, value_type: ctl_property_value_type_t, set: bool) -> ctl_3d_feature_getset_t {
    // SAFETY: all-zero is a valid value for this plain C structure (null pointers, zero lengths).
    let mut g: ctl_3d_feature_getset_t = unsafe { std::mem::zeroed() };
    g.Size = std::mem::size_of::<ctl_3d_feature_getset_t>() as u32;
    g.FeatureType = feature;
    g.ValueType = value_type;
    g.bSet = set;
    g
}

/// Fill a set block with `value`, checked against the control built from the driver's own caps.
pub(crate) fn encode(control: &GpuControl, d: &ctl_3d_feature_details_t, value: f64) -> anyhow::Result<ctl_3d_feature_getset_t> {
    if !control.supported {
        anyhow::bail!("{}", control.unsupported_reason.clone().unwrap_or_else(|| "This control is not supported".into()));
    }
    let mut g = getset(d.FeatureType, d.ValueType, true);
    match (&control.kind, d.ValueType) {
        (ControlKind::Toggle, CTL_PROPERTY_VALUE_TYPE_BOOL) => g.Value.BoolType = ctl_property_boolean_t { Enable: value != 0.0 },
        (ControlKind::Choice { options }, CTL_PROPERTY_VALUE_TYPE_ENUM) => {
            if !options.iter().any(|o| o.value == value) {
                anyhow::bail!("The driver does not offer that value for {}", control.name);
            }
            g.Value.EnumType = ctl_property_enum_t { EnableType: value as u32 };
        }
        (ControlKind::Range { min, max, .. }, t) => {
            if !value.is_finite() || value < *min || value > *max {
                anyhow::bail!("{} must be between {min} and {max}", control.name);
            }
            match t {
                CTL_PROPERTY_VALUE_TYPE_FLOAT => g.Value.FloatType = ctl_property_float_t { Enable: true, Value: value as f32 },
                CTL_PROPERTY_VALUE_TYPE_INT32 => g.Value.IntType = ctl_property_int_t { Enable: true, Value: value.round() as i32 },
                CTL_PROPERTY_VALUE_TYPE_UINT32 => g.Value.UIntType = ctl_property_uint_t { Enable: true, Value: value.round() as u32 },
                _ => anyhow::bail!("{} has a value type Arcadia does not edit", control.name),
            }
        }
        _ => anyhow::bail!("{} has a value type Arcadia does not edit", control.name),
    }
    Ok(g)
}

/// Unit label for a value IGCL reports. The unit differs between GPU
/// generations, so the value is always shown in the unit the driver names.
pub(crate) fn unit_label(units: ctl_units_t) -> &'static str {
    match units {
        CTL_UNITS_POWER_WATTS => "W",
        CTL_UNITS_POWER_MILLIWATTS => "mW",
        CTL_UNITS_TEMPERATURE_CELSIUS => "°C",
        CTL_UNITS_PERCENT => "%",
        CTL_UNITS_FREQUENCY_MHZ => "MHz",
        _ => "",
    }
}

/// One of the two limits IGCL's overclocking interface exposes that Arcadia
/// offers. Values stay in the driver's own unit; nothing is converted.
pub(crate) fn limit_control(id: &str, name: &str, description: &str, info: &ctl_oc_control_info_t, current: Option<f64>) -> GpuControl {
    let usable = info.bSupported && info.min.is_finite() && info.max.is_finite() && info.max > info.min;
    let unit = if info.bRelative && unit_label(info.units).is_empty() { "%" } else { unit_label(info.units) };
    GpuControl {
        id: id.to_string(),
        vendor: Vendor::Intel,
        name: name.to_string(),
        description: if info.bRelative { format!("{description} The driver expresses it relative to the default.") } else { description.to_string() },
        category: "Limits".into(),
        kind: if usable { ControlKind::Range { min: info.min, max: info.max, step: if info.step > 0.0 { info.step } else { 1.0 }, unit: unit.into() } } else { ControlKind::ReadOnly },
        value: current,
        default: usable.then_some(info.Default),
        supported: usable,
        unsupported_reason: (!usable).then(|| "The driver does not expose this limit for this adapter.".to_string()),
        risk: Risk::High,
        level: 3,
        requires_restart: false,
    }
}

#[cfg(windows)]
mod live {
    use super::*;
    use crate::frameforge::ffi::igcl::Igcl;
    use crate::frameforge::gpu::GpuDevice;

    const CTL_OK: ctl_result_t = 0;
    pub const POWER_LIMIT: &str = "igcl.power_limit";
    pub const TEMP_LIMIT: &str = "igcl.temperature_limit";

    fn caps(igcl: &Igcl, handle: ctl_device_adapter_handle_t) -> Vec<ctl_3d_feature_details_t> {
        let Ok(f) = &igcl.lib.ctlGetSupported3DCapabilities else { return Vec::new() };
        // SAFETY: all-zero is a valid value for this plain C structure.
        let mut c: ctl_3d_feature_caps_t = unsafe { std::mem::zeroed() };
        c.Size = std::mem::size_of::<ctl_3d_feature_caps_t>() as u32;
        // SAFETY: first call with a null array asks for the count, as in Intel's sample.
        if unsafe { f(handle, &mut c) } != CTL_OK || c.NumSupportedFeatures == 0 || c.NumSupportedFeatures > 256 {
            return Vec::new();
        }
        // SAFETY: all-zero is a valid ctl_3d_feature_details_t (null custom pointer, zero sizes).
        let mut details: Vec<ctl_3d_feature_details_t> = vec![unsafe { std::mem::zeroed() }; c.NumSupportedFeatures as usize];
        c.pFeatureDetails = details.as_mut_ptr();
        // SAFETY: the array has room for NumSupportedFeatures entries. Custom
        // blocks are left null, so the driver fills only the fixed part.
        if unsafe { f(handle, &mut c) } != CTL_OK {
            return Vec::new();
        }
        details.truncate(c.NumSupportedFeatures as usize);
        details
    }

    fn read(igcl: &Igcl, handle: ctl_device_adapter_handle_t, d: &ctl_3d_feature_details_t) -> Option<f64> {
        if d.ValueType == CTL_PROPERTY_VALUE_TYPE_CUSTOM {
            return None;
        }
        let f = igcl.lib.ctlGetSet3DFeature.as_ref().ok()?;
        let mut g = getset(d.FeatureType, d.ValueType, false);
        // SAFETY: sized block, no application name, no custom value.
        (unsafe { f(handle, &mut g) } == CTL_OK).then(|| value_of(&g)).flatten()
    }

    fn oc(igcl: &Igcl, handle: ctl_device_adapter_handle_t) -> Option<ctl_oc_properties_t> {
        let f = igcl.lib.ctlOverclockGetProperties.as_ref().ok()?;
        // SAFETY: all-zero is a valid value for this plain C structure.
        let mut p: ctl_oc_properties_t = unsafe { std::mem::zeroed() };
        p.Size = std::mem::size_of::<ctl_oc_properties_t>() as u32;
        // SAFETY: sized out-structure.
        (unsafe { f(handle, &mut p) } == CTL_OK && p.bSupported).then_some(p)
    }

    fn limit(f: &Result<unsafe extern "C" fn(ctl_device_adapter_handle_t, *mut f64) -> ctl_result_t, libloading::Error>, handle: ctl_device_adapter_handle_t) -> Option<f64> {
        let f = f.as_ref().ok()?;
        let mut v = 0f64;
        // SAFETY: valid out-pointer.
        (unsafe { f(handle, &mut v) } == CTL_OK && v.is_finite()).then_some(v)
    }

    pub(crate) fn list(igcl: &Igcl, device: &GpuDevice) -> Vec<GpuControl> {
        let Some(dev) = igcl.device_for(device) else { return Vec::new() };
        let handle = dev.handle as ctl_device_adapter_handle_t;
        let mut out: Vec<GpuControl> = caps(igcl, handle).iter().filter_map(|d| control_from_details(d, read(igcl, handle, d))).collect();
        if let Some(p) = oc(igcl, handle) {
            out.push(limit_control(POWER_LIMIT, "Sustained power limit", "The power the card may draw over time. Lower values reduce heat and noise; values above the default raise both.", &p.powerLimit, limit(&igcl.lib.ctlOverclockPowerLimitGetV2, handle)));
            out.push(limit_control(TEMP_LIMIT, "Temperature limit", "The temperature at which the card starts slowing itself down.", &p.temperatureLimit, limit(&igcl.lib.ctlOverclockTemperatureLimitGetV2, handle)));
        }
        out
    }

    fn fail(what: &str, st: ctl_result_t) -> anyhow::Error {
        match st as u32 {
            0x4000_0006 => anyhow::anyhow!("The Intel driver refused to change {what}: administrator rights are needed"),
            0x4000_000a => anyhow::anyhow!("The Intel driver does not support changing {what} on this adapter"),
            other => anyhow::anyhow!("The Intel driver could not change {what} (IGCL result 0x{other:08X})"),
        }
    }

    pub(crate) fn set(igcl: &Igcl, device: &GpuDevice, id: &str, value: f64) -> anyhow::Result<()> {
        let dev = igcl.device_for(device).ok_or_else(|| anyhow::anyhow!("IGCL does not list this adapter"))?;
        let handle = dev.handle as ctl_device_adapter_handle_t;
        if id == POWER_LIMIT || id == TEMP_LIMIT {
            let control = list(igcl, device).into_iter().find(|c| c.id == id).filter(|c| c.supported).ok_or_else(|| anyhow::anyhow!("The driver does not expose this limit for this adapter"))?;
            let ControlKind::Range { min, max, .. } = control.kind else { anyhow::bail!("The driver did not report a range for {}", control.name) };
            if !value.is_finite() || value < min || value > max {
                anyhow::bail!("{} must be between {min} and {max}", control.name);
            }
            let f = if id == POWER_LIMIT { &igcl.lib.ctlOverclockPowerLimitSetV2 } else { &igcl.lib.ctlOverclockTemperatureLimitSetV2 };
            let f = f.as_ref().map_err(|_| anyhow::anyhow!("This Intel driver does not implement the limit controls"))?;
            // SAFETY: plain value call on a valid adapter handle; the value is inside the driver-reported range.
            let st = unsafe { f(handle, value) };
            return if st == CTL_OK { Ok(()) } else { Err(fail(&control.name, st)) };
        }
        let key = id.strip_prefix("igcl.3d.").ok_or_else(|| anyhow::anyhow!("IGCL does not expose the control '{id}'"))?;
        let feature = feature_by_key(key).ok_or_else(|| anyhow::anyhow!("IGCL does not expose the control '{id}'"))?;
        let details = caps(igcl, handle);
        let d = details.iter().find(|d| d.FeatureType == feature.id).ok_or_else(|| anyhow::anyhow!("The driver does not list {} for this adapter", feature.name))?;
        let control = control_from_details(d, None).ok_or_else(|| anyhow::anyhow!("The driver reported an unusable range for {}", feature.name))?;
        let mut g = encode(&control, d, value)?;
        let f = igcl.lib.ctlGetSet3DFeature.as_ref().map_err(|_| anyhow::anyhow!("This Intel driver does not implement ctlGetSet3DFeature"))?;
        // SAFETY: sized block built from the driver's own capability entry; no application name, no custom value.
        let st = unsafe { f(handle, &mut g) };
        if st == CTL_OK {
            Ok(())
        } else {
            Err(fail(feature.name, st))
        }
    }
}

#[cfg(windows)]
pub(crate) use live::{list, set};

#[cfg(test)]
mod tests {
    use super::*;

    fn details(feature: ctl_3d_feature_t, value_type: ctl_property_value_type_t) -> ctl_3d_feature_details_t {
        // SAFETY: all-zero is a valid value for this plain C structure.
        let mut d: ctl_3d_feature_details_t = unsafe { std::mem::zeroed() };
        d.FeatureType = feature;
        d.ValueType = value_type;
        d
    }

    #[test]
    fn enum_features_offer_only_what_the_driver_lists() {
        let mut d = details(CTL_3D_FEATURE_ANISOTROPIC, CTL_PROPERTY_VALUE_TYPE_ENUM);
        // Application choice, 4x and 16x: bit N stands for value N.
        d.Value.EnumType = ctl_property_info_enum_t { SupportedTypes: (1 << 0) | (1 << 4) | (1 << 16), DefaultType: 0 };
        let c = control_from_details(&d, Some(4.0)).unwrap();
        assert_eq!(c.id, "igcl.3d.anisotropic");
        assert!(c.supported);
        let ControlKind::Choice { options } = &c.kind else { panic!("{:?}", c.kind) };
        assert_eq!(options.iter().map(|o| o.label.as_str()).collect::<Vec<_>>(), vec!["Application choice", "4x", "16x"]);
        assert_eq!((c.value, c.default), (Some(4.0), Some(0.0)));
        // A value the driver did not list is refused before any call is made.
        assert!(encode(&c, &d, 8.0).is_err());
        let g = encode(&c, &d, 16.0).unwrap();
        assert!(g.bSet && g.ApplicationName.is_null() && g.Size as usize == std::mem::size_of::<ctl_3d_feature_getset_t>());
        assert_eq!(value_of(&g), Some(16.0));
    }

    #[test]
    fn flag_style_enums_use_the_flags_themselves() {
        let mut d = details(CTL_3D_FEATURE_GAMING_FLIP_MODES, CTL_PROPERTY_VALUE_TYPE_ENUM);
        d.Value.EnumType = ctl_property_info_enum_t { SupportedTypes: 1 | 2 | 4 | 8, DefaultType: 1 };
        let c = control_from_details(&d, None).unwrap();
        let ControlKind::Choice { options } = &c.kind else { panic!() };
        assert_eq!(options.iter().map(|o| o.value).collect::<Vec<_>>(), vec![1.0, 2.0, 4.0, 8.0]);
        assert_eq!(c.value, None, "a value that could not be read stays unknown");
    }

    #[test]
    fn ranges_come_from_the_driver_and_are_enforced() {
        let mut d = details(CTL_3D_FEATURE_FRAME_LIMIT, CTL_PROPERTY_VALUE_TYPE_INT32);
        d.Value.IntType = ctl_property_info_int_t { DefaultEnable: false, RangeInfo: ctl_property_range_info_int_t { min_possible_value: 30, max_possible_value: 240, step_size: 1, default_value: 60 } };
        let c = control_from_details(&d, Some(120.0)).unwrap();
        assert_eq!(c.kind, ControlKind::Range { min: 30.0, max: 240.0, step: 1.0, unit: "FPS".into() });
        assert!(encode(&c, &d, 500.0).is_err());
        assert!(encode(&c, &d, f64::NAN).is_err());
        assert_eq!(value_of(&encode(&c, &d, 144.0).unwrap()), Some(144.0));
        // A nonsense range from the driver yields no control at all.
        d.Value.IntType.RangeInfo.max_possible_value = 10;
        assert!(control_from_details(&d, None).is_none());
    }

    #[test]
    fn structured_and_unknown_features_are_never_editable() {
        let c = control_from_details(&details(CTL_3D_FEATURE_ENDURANCE_GAMING, CTL_PROPERTY_VALUE_TYPE_CUSTOM), Some(1.0)).unwrap();
        assert!(!c.supported && c.value.is_none() && c.unsupported_reason.is_some());
        let d = details(CTL_3D_FEATURE_LOW_LATENCY, CTL_PROPERTY_VALUE_TYPE_CUSTOM);
        let c = control_from_details(&d, None).unwrap();
        assert!(!c.supported);
        assert!(encode(&c, &d, 1.0).is_err());
        // A feature id Arcadia has no entry for is not shown as a control.
        assert!(control_from_details(&details(CTL_3D_FEATURE_APP_PROFILES, CTL_PROPERTY_VALUE_TYPE_ENUM), None).is_none());
        // A known enum feature with no known values supported is shown as unsupported.
        let d = details(CTL_3D_FEATURE_CMAA, CTL_PROPERTY_VALUE_TYPE_ENUM);
        assert!(!control_from_details(&d, None).unwrap().supported);
    }

    #[test]
    fn limits_are_expert_controls_inside_the_reported_range() {
        // SAFETY: all-zero is a valid value for this plain C structure.
        let mut info: ctl_oc_control_info_t = unsafe { std::mem::zeroed() };
        let c = limit_control("igcl.power_limit", "Sustained power limit", "", &info, None);
        assert!(!c.supported && c.kind == ControlKind::ReadOnly);
        info.bSupported = true;
        info.min = 100_000.0;
        info.max = 228_000.0;
        info.step = 1_000.0;
        info.Default = 190_000.0;
        info.units = CTL_UNITS_POWER_MILLIWATTS;
        let c = limit_control("igcl.power_limit", "Sustained power limit", "", &info, Some(190_000.0));
        // The driver's own unit and numbers, unconverted.
        assert_eq!(c.kind, ControlKind::Range { min: 100_000.0, max: 228_000.0, step: 1_000.0, unit: "mW".into() });
        assert_eq!((c.value, c.default, c.level, c.risk), (Some(190_000.0), Some(190_000.0), 3, Risk::High));
    }
}
