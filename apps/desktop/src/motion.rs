use gpui::{App, Global};

struct AlwaysReduce(bool);

impl Global for AlwaysReduce {}

pub fn always_reduce(cx: &App) -> bool {
    cx.try_global::<AlwaysReduce>()
        .is_some_and(|preference| preference.0)
}

pub fn set_always_reduce(cx: &mut App, reduced: bool) {
    cx.set_global(AlwaysReduce(reduced));
    apply_preference(cx);
}

pub fn enabled(cx: &App) -> bool {
    !cx.reduce_motion() && !system_reduces_motion()
}

pub fn sync_from_system(cx: &mut App) {
    #[cfg(not(test))]
    apply_preference(cx);

    #[cfg(test)]
    let _ = cx;
}

fn apply_preference(cx: &mut App) {
    let reduce = always_reduce(cx) || system_reduces_motion();
    cx.set_reduce_motion(reduce);
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
