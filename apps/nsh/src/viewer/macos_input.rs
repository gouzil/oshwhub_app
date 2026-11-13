//! Canvas-local AppKit input; all camera updates run through NSH's engine queue.

use std::rc::Rc;

use nsh_core::{element::AnyElement, prelude::*};
use nsh_platform_macos::{
    ComponentContext, Components, NativeComponent,
    objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained},
    objc2_app_kit::{NSApplication, NSEvent, NSEventType, NSView},
    objc2_foundation::{NSObjectProtocol, NSPoint},
};

define_class!(
    #[unsafe(super(NSView))]
    #[name = "OSHWHubWheelView"]
    #[thread_kind = MainThreadOnly]
    #[ivars = ComponentContext]
    struct WheelView;

    unsafe impl NSObjectProtocol for WheelView {}

    impl WheelView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool { true }

        #[unsafe(method_id(hitTest:))]
        fn hit_test(&self, point: NSPoint) -> Option<Retained<NSView>> {
            let captures = NSApplication::sharedApplication(self.mtm()).currentEvent()
                .is_some_and(|event| matches!(event.r#type(), NSEventType::ScrollWheel | NSEventType::Magnify));
            if captures {
                // SAFETY: NSView checks its bounds; all other input returns to NSH's root picker.
                unsafe { msg_send![super(self), hitTest: point] }
            } else {
                None
            }
        }

        #[unsafe(method(scrollWheel:))]
        fn scroll_wheel(&self, event: &NSEvent) {
            let unit = if event.hasPreciseScrollingDeltas() { 1. } else { 40. };
            self.emit(-event.scrollingDeltaY() * unit, event);
        }

        #[unsafe(method(magnifyWithEvent:))]
        fn magnify(&self, event: &NSEvent) {
            let amount = event.magnification();
            if amount > -1. {
                self.emit(-amount.ln_1p() / 0.003, event);
            }
        }
    }
);

impl WheelView {
    fn emit(&self, delta: f64, event: &NSEvent) {
        let at = self.convertPoint_fromView(event.locationInWindow(), None);
        if [delta, at.x, at.y].into_iter().all(f64::is_finite) {
            self.ivars().emit(
                "wheel",
                serde_json::json!({"delta":delta,"x":at.x,"y":at.y}),
            );
        }
    }
}

struct WheelComponent(Retained<WheelView>);

impl NativeComponent for WheelComponent {
    fn view(&self) -> Retained<NSView> {
        Retained::into_super(self.0.clone())
    }
    fn set_prop(&mut self, _name: &str, _value: Option<&nsh_core::JsonValue>) {}
}

pub fn register(components: &Components) {
    components.register(
        "oshwhub.wheel",
        Rc::new(|context| {
            let mtm =
                MainThreadMarker::new().expect("NSH creates AppKit components on its main thread");
            let view = WheelView::alloc(mtm).set_ivars(context);
            // SAFETY: NSView initializes the allocated WheelView and its retained context.
            let view = unsafe { msg_send![super(view), init] };
            Box::new(WheelComponent(view))
        }),
    );
}

pub fn overlay(scene_id: String, wheel: fn(&str, f64, f64, f64) -> bool) -> AnyElement {
    native("oshwhub.wheel")
        .absolute()
        .left_0()
        .right_0()
        .top_0()
        .bottom_0()
        .accessibility_elements_hidden(true)
        .on_native_event(move |event: &NativeEvent| {
            if event.name.as_ref() == "wheel"
                && let (Some(delta), Some(x), Some(y)) = (
                    event.data["delta"].as_f64(),
                    event.data["x"].as_f64(),
                    event.data["y"].as_f64(),
                )
            {
                wheel(&scene_id, delta, x, y);
            }
        })
        .into_element()
}
