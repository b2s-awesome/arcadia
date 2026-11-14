#!/bin/sh
# Regenerate FrameForge's vendor type bindings with rust-bindgen.
#
#   tools/frameforge/bindgen.sh nvapi <NVAPI SDK checkout>      > nvapi.rs
#   tools/frameforge/bindgen.sh igcl  <IGCL checkout>           > igcl.rs
#
# Needs `bindgen` (cargo install bindgen-cli) and libclang. The output goes
# to stdout; review the diff, keep the licence header at the top of the
# committed file, then replace crates/core/src/frameforge/ffi/<name>_types.rs.
#
# ADLX is not generated here: its licence forbids committing header-derived
# source, so crates/core/build.rs generates it at build time (feature `adlx`).
#
# presentmon_types.rs was generated the same way from PresentMonAPI2's
# PresentMonAPI.h (--dynamic-loading PmLib, consts enum style). The exact
# allowlist was not kept; if you regenerate it, compare against the committed
# file before replacing it.
set -eu
here=$(cd "$(dirname "$0")" && pwd)
kind=${1:?nvapi or igcl}
sdk=${2:?path to the SDK checkout}
common="--default-enum-style consts --no-doc-comments --no-prepend-enum-name --with-derive-default --rust-target 1.82 --use-core"

case "$kind" in
  nvapi)
    # shellcheck disable=SC2086
    bindgen "$here/nvapi_wrapper.h" $common --no-derive-debug \
      --allowlist-type 'NVDRS_.*' --allowlist-type 'NvDRS.*' --allowlist-type '_?NvAPI_Status' \
      --allowlist-type 'NV_GPU_THERMAL_SETTINGS.*' --allowlist-type 'NV_GPU_DYNAMIC_PSTATES_INFO_EX' \
      --allowlist-type 'NV_GPU_CLOCK_FREQUENCIES.*' --allowlist-type 'NV_GPU_MEMORY_INFO_EX.*' \
      --allowlist-type 'NV_GPU_TYPE' --allowlist-type 'NV_SYSTEM_TYPE' --allowlist-type 'NV_GPU_ARCH_INFO.*' \
      --allowlist-type 'NV_GET_VRR_INFO.*' --allowlist-type 'NV_GPU_DISPLAYIDS' --allowlist-type 'NV_DISPLAY_DRIVER_INFO' \
      --allowlist-type 'NvPhysicalGpuHandle' --allowlist-type 'NvAPI_ShortString' --allowlist-type 'NvAPI_UnicodeString' \
      --allowlist-var 'NVAPI_.*MAX.*' --allowlist-var 'NVDRS_.*' \
      -- -x c++ --target=x86_64-pc-windows-msvc "-I$sdk" -fms-extensions -fms-compatibility
    ;;
  igcl)
    funcs="ctlInit ctlClose ctlEnumerateDevices ctlGetDeviceProperties ctlCheckDriverVersion ctlPowerTelemetryGet ctlEnumTemperatureSensors ctlTemperatureGetState ctlTemperatureGetProperties ctlEnumFans ctlFanGetProperties ctlFanGetConfig ctlFanGetState ctlEnumMemoryModules ctlMemoryGetState ctlEnumFrequencyDomains ctlFrequencyGetState ctlGetSupported3DCapabilities ctlGetSet3DFeature ctlOverclockGetProperties ctlOverclockWaiverSet ctlOverclockResetToDefault ctlOverclockPowerLimitGetV2 ctlOverclockPowerLimitSetV2 ctlOverclockTemperatureLimitGetV2 ctlOverclockTemperatureLimitSetV2 ctlOverclockGpuFrequencyOffsetGetV2 ctlOverclockGpuFrequencyOffsetSetV2 ctlEnumerateDisplayOutputs ctlGetDisplayProperties ctlGetAdaperDisplayEncoderProperties"
    args=""
    for f in $funcs; do args="$args --allowlist-function ^$f\$"; done
    # shellcheck disable=SC2086
    bindgen "$sdk/include/igcl_api.h" $common $args --dynamic-loading IgclLib \
      --allowlist-var 'CTL_IMPL_.*' --allowlist-var 'CTL_MAX_.*' --allowlist-type 'ctl_3d_.*' \
      --allowlist-type 'ctl_gaming_flip_mode_flag_t' --allowlist-type 'ctl_endurance_gaming.*' --allowlist-type 'ctl_3d_low_latency_types_t' \
      -- -x c++ --target=x86_64-pc-windows-msvc -fms-extensions
    ;;
  *)
    echo "unknown binding '$kind' (nvapi or igcl)" >&2
    exit 2
    ;;
esac
