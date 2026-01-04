//! Tests for page! macro handler generation.
//!
//! These tests verify the handler generation pattern compiles correctly.
//! Full integration tests with real widgets will be in rafter/tests/.

use tuidom::Element;

// Re-use rafter's actual types
use rafter::{AppContext, GlobalContext, State};

// Mock widget that mimics what a real widget would look like
#[derive(Clone)]
struct TestButton {
    label: String,
    on_click: Option<std::sync::Arc<dyn Fn(&AppContext, &GlobalContext) + Send + Sync>>,
}

impl TestButton {
    fn new() -> Self {
        TestButton {
            label: String::new(),
            on_click: None,
        }
    }

    fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    fn on_click<F>(mut self, handler: F) -> Self
    where
        F: Fn(&AppContext, &GlobalContext) + Send + Sync + 'static,
    {
        self.on_click = Some(std::sync::Arc::new(handler));
        self
    }

    fn element(self) -> Element {
        Element::text(&self.label)
    }
}

// Mock app using State<T> like a real #[app] would have
#[derive(Clone)]
struct TestApp {
    counter: State<i32>,
}

impl TestApp {
    fn new() -> Self {
        TestApp {
            counter: State::new(0),
        }
    }

    fn handle_click(&self) {
        self.counter.update(|v| *v += 1);
    }

    fn handle_click_with_arg(&self, value: i32) {
        self.counter.set(value);
    }

    fn handle_with_context(&self, _cx: &AppContext) {
        self.counter.update(|v| *v += 1);
    }

    fn handle_with_both_contexts(&self, _cx: &AppContext, _gx: &GlobalContext) {
        self.counter.update(|v| *v += 1);
    }

    fn handle_mixed(&self, value: i32, _cx: &AppContext) {
        self.counter.set(value);
    }
}

// These tests verify the code pattern that page! generates compiles correctly.
// The pattern is:
//   let __self = self.clone();
//   let __arg0 = (expr).clone();
//   Widget::new()
//       .on_click(move |cx, gx| { __self.handler(__arg0, cx); })
//       .element()

#[test]
fn test_handler_no_args() {
    let app = TestApp::new();

    // Simulates: button (label: "Click") on_click: handle_click()
    let _elem = {
        let __self = app.clone();
        TestButton::new()
            .label("Click")
            .on_click(move |_cx: &AppContext, _gx: &GlobalContext| {
                __self.handle_click();
            })
            .element()
    };
}

#[test]
fn test_handler_with_captured_arg() {
    let app = TestApp::new();
    let item_id = 42;

    // Simulates: button (label: "Delete") on_click: handle_click_with_arg(item_id)
    let _elem = {
        let __self = app.clone();
        let __arg0 = (item_id).clone();
        TestButton::new()
            .label("Delete")
            .on_click(move |_cx: &AppContext, _gx: &GlobalContext| {
                let __arg0 = __arg0.clone();
                __self.handle_click_with_arg(__arg0);
            })
            .element()
    };
}

#[test]
fn test_handler_with_context_arg() {
    let app = TestApp::new();

    // Simulates: button (label: "Click") on_click: handle_with_context(cx)
    let _elem = {
        let __self = app.clone();
        TestButton::new()
            .label("Click")
            .on_click(move |cx: &AppContext, _gx: &GlobalContext| {
                __self.handle_with_context(cx);
            })
            .element()
    };
}

#[test]
fn test_handler_with_both_contexts() {
    let app = TestApp::new();

    // Simulates: button (label: "Click") on_click: handle_with_both_contexts(cx, gx)
    let _elem = {
        let __self = app.clone();
        TestButton::new()
            .label("Click")
            .on_click(move |cx: &AppContext, gx: &GlobalContext| {
                __self.handle_with_both_contexts(cx, gx);
            })
            .element()
    };
}

#[test]
fn test_handler_with_mixed_args() {
    let app = TestApp::new();
    let item_id = 42;

    // Simulates: button (label: "Update") on_click: handle_mixed(item_id, cx)
    let _elem = {
        let __self = app.clone();
        let __arg0 = (item_id).clone();
        TestButton::new()
            .label("Update")
            .on_click(move |cx: &AppContext, _gx: &GlobalContext| {
                let __arg0 = __arg0.clone();
                __self.handle_mixed(__arg0, cx);
            })
            .element()
    };
}
