fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS");
    if target_os.as_deref() == Ok("macos") {
        println!("cargo:rerun-if-changed=src/capture_drag_macos.m");
        cc::Build::new()
            .file("src/capture_drag_macos.m")
            .flag("-fobjc-arc")
            .compile("snapshoot_capture_drag");
        println!("cargo:rerun-if-changed=src/screen_record/macos_record.m");
        cc::Build::new()
            .file("src/screen_record/macos_record.m")
            .flag("-fobjc-arc")
            .flag("-fblocks")
            .flag("-Wunguarded-availability")
            .compile("snapshoot_screen_record");
        println!("cargo:rerun-if-changed=src/vision_ocr.m");
        cc::Build::new()
            .file("src/vision_ocr.m")
            .flag("-fobjc-arc")
            .flag("-fblocks")
            .flag("-Wunguarded-availability")
            .compile("snapshoot_vision_ocr");
        // Keep the existing deployment target; ScreenCaptureKit is optional at load.
        println!("cargo:rustc-link-arg=-Wl,-weak_framework,ScreenCaptureKit");
        for framework in [
            "AVFoundation",
            "CoreMedia",
            "CoreVideo",
            "AudioToolbox",
            "CoreGraphics",
            "Foundation",
            "AppKit",
            "ImageIO",
            "Vision",
        ] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
    }
    if target_os.as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=src/capture_drag_windows.cpp");
        cc::Build::new()
            .cpp(true)
            .file("src/capture_drag_windows.cpp")
            .compile("snapshoot_capture_drag");
        for library in ["ole32", "shell32", "user32", "uuid"] {
            println!("cargo:rustc-link-lib={library}");
        }
    }
    tauri_build::build();
}
