//! IGCL driver controls. Implemented in the multi-vendor slice.

use super::igcl::Igcl;
use crate::frameforge::gpu::{GpuControl, GpuDevice};

pub(crate) fn list(_igcl: &Igcl, _device: &GpuDevice) -> Vec<GpuControl> {
    Vec::new()
}

pub(crate) fn set(_igcl: &Igcl, _device: &GpuDevice, id: &str, _value: f64) -> anyhow::Result<()> {
    anyhow::bail!("IGCL does not expose the control '{id}'")
}
