fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rerun-if-changed=src/screen_record/macos_record.m");
        cc::Build::new()
            .file("src/screen_record/macos_record.m")
            .flag("-fobjc-arc")
            .flag("-fblocks")
            .flag("-Wunguarded-availability")
            .compile("snapshoot_screen_record");
        // Keep the existing deployment target; ScreenCaptureKit is optional at load.
        println!("cargo:rustc-link-arg=-Wl,-weak_framework,ScreenCaptureKit");
        for framework in ["AVFoundation", "CoreMedia", "CoreVideo", "AudioToolbox", "CoreGraphics", "Foundation", "AppKit"] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
    }
    tauri_build::build();
}
