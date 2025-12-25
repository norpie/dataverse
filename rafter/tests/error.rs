//! Tests for app error types.

use std::any::Any;

use rafter::app::{AppError, AppErrorKind, InstanceId, extract_panic_message};

#[test]
fn test_extract_panic_message_str() {
    let panic: Box<dyn Any + Send> = Box::new("test panic message");
    assert_eq!(extract_panic_message(&panic), "test panic message");
}

#[test]
fn test_extract_panic_message_string() {
    let panic: Box<dyn Any + Send> = Box::new(String::from("test panic message"));
    assert_eq!(extract_panic_message(&panic), "test panic message");
}

#[test]
fn test_extract_panic_message_unknown() {
    let panic: Box<dyn Any + Send> = Box::new(42i32);
    assert_eq!(extract_panic_message(&panic), "Unknown panic");
}

#[test]
fn test_app_error_display() {
    let error = AppError {
        app_name: "TestApp",
        instance_id: InstanceId::new(),
        kind: AppErrorKind::Panic {
            handler_name: "do_something".to_string(),
            message: "oops".to_string(),
        },
    };
    let display = format!("{}", error);
    assert!(display.contains("TestApp"));
    assert!(display.contains("do_something"));
    assert!(display.contains("oops"));
}
