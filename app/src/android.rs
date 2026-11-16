//! Android platform bridge: calls into `MainActivity` (Java) over JNI.
//!
//! Kept free of Android-only types (it only needs the JavaVM and activity
//! pointers), so it also type-checks on desktop. Used only on Android.

use jni::objects::{JClass, JObject, JObjectArray, JString, JValue};
use jni::sys;
use jni::{JNIEnv, JavaVM, NativeMethod};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicI32, Ordering};

/// Asks the UI to draw a frame; set once the egui context exists.
static WAKER: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();
/// Page steps from the volume keys, not yet consumed (+ = forward).
static PAGE_STEPS: AtomicI32 = AtomicI32::new(0);

fn wake() {
    if let Some(w) = WAKER.get() {
        w();
    }
}

extern "system" fn native_wake(_env: JNIEnv, _class: JClass) {
    wake();
}

extern "system" fn native_page_step(_env: JNIEnv, _class: JClass, direction: sys::jint) {
    PAGE_STEPS.fetch_add(direction.signum(), Ordering::SeqCst);
    wake();
}

/// Let Java wake the UI (e.g. when a picked document arrives or a volume key is pressed).
pub fn set_waker(f: Box<dyn Fn() + Send + Sync>) {
    let _ = WAKER.set(f);
}

/// Net page steps requested with the volume keys since the last call.
pub fn take_page_steps() -> i32 {
    PAGE_STEPS.swap(0, Ordering::SeqCst)
}

/// (JavaVM*, activity jobject) from android-activity.
static PTRS: OnceLock<(usize, usize)> = OnceLock::new();

pub fn init(vm: *mut core::ffi::c_void, activity: *mut core::ffi::c_void) {
    let _ = PTRS.set((vm as usize, activity as usize));
    // Java -> Rust callbacks. If this fails the app still works: it polls for
    // documents and the volume keys keep changing the volume.
    with_activity(|env, a| {
        let class = env.get_object_class(a)?;
        env.register_native_methods(
            &class,
            &[
                NativeMethod {
                    name: "nativeWake".into(),
                    sig: "()V".into(),
                    fn_ptr: native_wake as *mut core::ffi::c_void,
                },
                NativeMethod {
                    name: "nativePageStep".into(),
                    sig: "(I)V".into(),
                    fn_ptr: native_page_step as *mut core::ffi::c_void,
                },
            ],
        )
    });
}

fn call_bool(method: &str, on: bool) {
    with_activity(|env, a| env.call_method(a, method, "(Z)V", &[JValue::Bool(on as u8)]).map(|_| ()));
}

/// Volume keys turn pages (true) or change the volume (false).
pub fn set_volume_keys(on: bool) {
    call_bool("setVolumeKeys", on);
}

/// Keep the screen on (while a book is open).
pub fn set_keep_screen_on(on: bool) {
    call_bool("setKeepScreenOn", on);
}

/// A light haptic tick.
pub fn haptic_tick() {
    with_activity(|env, a| env.call_method(a, "hapticTick", "()V", &[]).map(|_| ()));
}

/// Run `f` with a JNI env attached to this thread and the activity object.
fn with_activity<T>(f: impl FnOnce(&mut jni::JNIEnv, &JObject) -> jni::errors::Result<T>) -> Option<T> {
    let (vm, act) = *PTRS.get()?;
    // SAFETY: pointers come from android-activity and live as long as the process / activity.
    let vm = unsafe { JavaVM::from_raw(vm as *mut sys::JavaVM) }.ok()?;
    let mut env = vm.attach_current_thread_permanently().ok()?;
    // SAFETY: the activity is a global reference owned by NativeActivity; we never delete it.
    let activity = unsafe { JObject::from_raw(act as sys::jobject) };
    let out = f(&mut env, &activity);
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
    }
    if let Err(e) = &out {
        log::warn!("JNI call failed: {e}");
    }
    out.ok()
}

/// Show the system document picker (results arrive via [`take_documents`]).
pub fn pick_documents() {
    with_activity(|env, a| env.call_method(a, "pickDocuments", "()V", &[]).map(|_| ()));
}

/// A document handed to us by the picker or "Open with".
pub struct Picked {
    pub uri: String,
    pub name: String,
}

/// Documents picked/opened since the last call.
pub fn take_documents() -> Vec<Picked> {
    with_activity(|env, a| {
        let arr = env.call_method(a, "takeDocuments", "()[Ljava/lang/String;", &[])?.l()?;
        let arr = JObjectArray::from(arr);
        let n = env.get_array_length(&arr)?;
        let mut out = Vec::with_capacity(n as usize);
        for i in 0..n {
            let s = JString::from(env.get_object_array_element(&arr, i)?);
            let s: String = env.get_string(&s)?.into();
            let (uri, name) = s.split_once('\n').unwrap_or((s.as_str(), "document"));
            out.push(Picked {
                uri: uri.to_owned(),
                name: name.to_owned(),
            });
        }
        Ok(out)
    })
    .unwrap_or_default()
}

/// Open a content URI for reading; returns a path the rest of the app can open
/// like a normal file (`/proc/self/fd/N`). The descriptor stays open for the
/// life of the process (a handful per session).
pub fn open_uri(uri: &str) -> Option<PathBuf> {
    let fd = with_activity(|env, a| {
        let s = env.new_string(uri)?;
        env.call_method(a, "openFd", "(Ljava/lang/String;)I", &[JValue::Object(&s)])?.i()
    })?;
    if fd < 0 {
        log::warn!("could not open {uri}");
    }
    (fd >= 0).then(|| PathBuf::from(format!("/proc/self/fd/{fd}")))
}

/// Close a descriptor obtained through [`open_uri`] once it is no longer needed.
pub fn close_path(path: &std::path::Path) {
    if let Some(fd) = path
        .to_str()
        .and_then(|s| s.strip_prefix("/proc/self/fd/"))
        .and_then(|n| n.parse::<i32>().ok())
    {
        #[cfg(unix)]
        {
            use std::os::fd::FromRawFd;
            // SAFETY: we own this descriptor (detached from a ParcelFileDescriptor).
            drop(unsafe { std::fs::File::from_raw_fd(fd) });
        }
    }
}

/// Colour the status and navigation bars.
pub fn set_bar_colors(rgb: [u8; 3], light_background: bool) {
    let argb = (0xFFu32 << 24 | (rgb[0] as u32) << 16 | (rgb[1] as u32) << 8 | rgb[2] as u32) as i32;
    with_activity(|env, a| {
        env.call_method(
            a,
            "setBarColors",
            "(IZ)V",
            &[JValue::Int(argb), JValue::Bool(light_background as u8)],
        )
        .map(|_| ())
    });
}
