//! Android entry point. Built with `cargo ndk` into `libghostleaf.so`; on other
//! platforms this crate is empty.

/// Called by android-activity (NativeActivity glue) on the app's main thread.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    ghostleaf_app::run_android(app);
}
