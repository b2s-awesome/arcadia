//! Build script. It does nothing unless the `adlx` feature is on.
//!
//! With `--features adlx` it generates Rust bindings for AMD's ADLX C
//! interface from an SDK checkout you supply:
//!
//! ```text
//! git clone https://github.com/GPUOpen-LibrariesAndSDKs/ADLX
//! set ADLX_SDK_DIR=C:\path\to\ADLX        (the folder that contains SDK\Include)
//! cargo build --features adlx
//! ```
//!
//! AMD's SDK licence allows its headers, and works derived from them, to be
//! distributed in object-code form only. That is why the bindings are made
//! on your machine at build time and are never committed to this repository.
//! Generating them needs libclang (install LLVM and, if it is not found,
//! set LIBCLANG_PATH).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=ADLX_SDK_DIR");
    #[cfg(feature = "adlx")]
    adlx::generate();
}

#[cfg(feature = "adlx")]
mod adlx {
    use std::path::PathBuf;

    /// Interfaces Arcadia calls. Everything else in the SDK is left out.
    const INTERFACES: &str = "IADLX(System|GPUList|GPU|PerformanceMonitoringServices|GPUMetrics|GPUMetricsSupport|3DSettingsServices|3DAntiLag|3DChill|3DBoost|3DImageSharpening|3DEnhancedSync|3DWaitForVerticalRefresh|3DFrameRateTargetControl|3DRadeonSuperResolution)(Vtbl)?";

    pub fn generate() {
        let sdk = std::env::var("ADLX_SDK_DIR").unwrap_or_else(|_| panic!("\n\nThe `adlx` feature needs ADLX_SDK_DIR set to a checkout of https://github.com/GPUOpen-LibrariesAndSDKs/ADLX\n\n"));
        let include = PathBuf::from(&sdk).join("SDK").join("Include");
        assert!(include.join("ADLX.h").is_file(), "ADLX_SDK_DIR={sdk} has no SDK/Include/ADLX.h");
        let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));

        // The ADLX headers include <Windows.h> but only need a handful of
        // names from it. A tiny stand-in keeps the generation independent of
        // the Windows SDK and identical on every build machine.
        let stub = out.join("adlx-stub");
        std::fs::create_dir_all(&stub).expect("stub dir");
        std::fs::write(stub.join("Windows.h"), "#pragma once\n#include <stddef.h>\ntypedef wchar_t TCHAR;\ntypedef void* HMODULE;\ntypedef void* HANDLE;\ntypedef void* HWND;\n").expect("stub");
        std::fs::write(stub.join("stdio.h"), "#pragma once\n").expect("stub");
        let wrapper = out.join("adlx_wrapper.h");
        std::fs::write(&wrapper, "#include \"ADLX.h\"\n#include \"ISystem.h\"\n#include \"IPerformanceMonitoring.h\"\n#include \"I3DSettings.h\"\n").expect("wrapper");

        let bindings = bindgen::Builder::default()
            .header(wrapper.to_string_lossy())
            .allowlist_type(INTERFACES)
            .allowlist_type("ADLX_.*")
            .allowlist_var("ADLX_.*")
            .default_enum_style(bindgen::EnumVariation::Consts)
            .prepend_enum_name(false)
            .generate_comments(false)
            .derive_default(true)
            .layout_tests(false)
            .use_core()
            // The C form of the headers: plain structs with function-pointer tables.
            .clang_args(["-x", "c", "--target=x86_64-pc-windows-msvc", "-fms-extensions", "-ffreestanding"])
            .clang_arg(format!("-I{}", stub.display()))
            .clang_arg(format!("-I{}", include.display()))
            .generate()
            .expect("could not generate ADLX bindings (is libclang installed?)");
        bindings.write_to_file(out.join("adlx_bindings.rs")).expect("write bindings");
        for h in ["ADLX.h", "ADLXDefines.h", "ISystem.h", "IPerformanceMonitoring.h", "I3DSettings.h"] {
            println!("cargo:rerun-if-changed={}", include.join(h).display());
        }
    }
}
