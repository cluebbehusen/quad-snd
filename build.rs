fn main() {
    let target = std::env::var("CARGO_CFG_TARGET_OS");

    if target.as_deref() == Ok("macos")
        || target.as_deref() == Ok("ios")
        || target.as_deref() == Ok("tvos")
    {
        println!("cargo:rustc-link-lib=framework=CoreAudio");
        println!("cargo:rustc-link-lib=framework=AudioToolBox");
    }

    // The audio session exists only on iOS and tvOS. It lives in AVFoundation
    // and is reached through the Objective-C runtime.
    if target.as_deref() == Ok("ios") || target.as_deref() == Ok("tvos") {
        println!("cargo:rustc-link-lib=framework=AVFoundation");
        println!("cargo:rustc-link-lib=objc");
    }
}
