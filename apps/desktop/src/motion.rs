use gpui::App;

pub fn enabled(cx: &App) -> bool {
    !cx.reduce_motion() && !system_reduces_motion()
}

pub fn sync_from_system(cx: &mut App) {
    #[cfg(all(target_os = "macos", not(test)))]
    cx.set_reduce_motion(system_reduces_motion());

    #[cfg(any(not(target_os = "macos"), test))]
    let _ = cx;
}

#[cfg(all(target_os = "macos", not(test)))]
fn system_reduces_motion() -> bool {
    use objc2::{
        MainThreadMarker, class, msg_send,
        runtime::{AnyObject, Bool},
    };

    if MainThreadMarker::new().is_none() {
        return true;
    }
    let workspace: &AnyObject = unsafe { msg_send![class!(NSWorkspace), sharedWorkspace] };
    let reduced: Bool = unsafe { msg_send![workspace, accessibilityDisplayShouldReduceMotion] };
    reduced.as_bool()
}

// Headless tests set the preference on the GPUI context.
#[cfg(any(not(target_os = "macos"), test))]
fn system_reduces_motion() -> bool {
    false
}
