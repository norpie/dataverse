use std::collections::HashSet;
use std::time::Duration;

use tuidom::animation::{collect_element_ids, AnimationState, PropertyValue, TransitionProperty};
use tuidom::{Color, Easing, Element, Style, TransitionConfig, Transitions};

// =============================================================================
// Easing Function Tests
// =============================================================================

#[test]
fn test_easing_linear() {
    assert_eq!(Easing::Linear.apply(0.0), 0.0);
    assert_eq!(Easing::Linear.apply(0.5), 0.5);
    assert_eq!(Easing::Linear.apply(1.0), 1.0);
    assert_eq!(Easing::Linear.apply(0.25), 0.25);
}

#[test]
fn test_easing_ease_in() {
    // EaseIn: t * t (quadratic)
    assert_eq!(Easing::EaseIn.apply(0.0), 0.0);
    assert_eq!(Easing::EaseIn.apply(1.0), 1.0);
    // At t=0.5, should be 0.25 (slower start)
    assert_eq!(Easing::EaseIn.apply(0.5), 0.25);
    // At t=0.25, should be 0.0625
    assert!((Easing::EaseIn.apply(0.25) - 0.0625).abs() < 0.0001);
}

#[test]
fn test_easing_ease_out() {
    // EaseOut: 1 - (1-t)^2 (quadratic, fast start)
    assert_eq!(Easing::EaseOut.apply(0.0), 0.0);
    assert_eq!(Easing::EaseOut.apply(1.0), 1.0);
    // At t=0.5, should be 0.75 (faster start)
    assert_eq!(Easing::EaseOut.apply(0.5), 0.75);
}

#[test]
fn test_easing_ease_in_out() {
    // EaseInOut: slow start, fast middle, slow end
    assert_eq!(Easing::EaseInOut.apply(0.0), 0.0);
    assert_eq!(Easing::EaseInOut.apply(1.0), 1.0);
    // At t=0.5, should be 0.5 (middle)
    assert_eq!(Easing::EaseInOut.apply(0.5), 0.5);
    // First half is slower (ease in)
    assert!(Easing::EaseInOut.apply(0.25) < 0.25);
    // Second half is faster (ease out)
    assert!(Easing::EaseInOut.apply(0.75) > 0.75);
}

#[test]
fn test_easing_boundaries() {
    // All easing functions should map 0->0 and 1->1
    for easing in [
        Easing::Linear,
        Easing::EaseIn,
        Easing::EaseOut,
        Easing::EaseInOut,
    ] {
        assert_eq!(easing.apply(0.0), 0.0, "{:?} at 0", easing);
        assert_eq!(easing.apply(1.0), 1.0, "{:?} at 1", easing);
    }
}

#[test]
fn test_easing_monotonic() {
    // All easing functions should be monotonically increasing
    for easing in [
        Easing::Linear,
        Easing::EaseIn,
        Easing::EaseOut,
        Easing::EaseInOut,
    ] {
        let mut prev = 0.0;
        for i in 1..=10 {
            let t = i as f32 / 10.0;
            let val = easing.apply(t);
            assert!(val >= prev, "{:?} not monotonic at t={}", easing, t);
            prev = val;
        }
    }
}

// =============================================================================
// TransitionConfig Tests
// =============================================================================

#[test]
fn test_transition_config_new() {
    let config = TransitionConfig::new(Duration::from_millis(300), Easing::EaseOut);
    assert_eq!(config.duration, Duration::from_millis(300));
    assert_eq!(config.easing, Easing::EaseOut);
}

// =============================================================================
// Transitions Builder Tests
// =============================================================================

#[test]
fn test_transitions_default_empty() {
    let t = Transitions::new();
    assert!(!t.has_any());
    assert!(t.left.is_none());
    assert!(t.background.is_none());
}

#[test]
fn test_transitions_individual_properties() {
    let t = Transitions::new()
        .left(Duration::from_millis(100), Easing::Linear)
        .background(Duration::from_millis(200), Easing::EaseIn);

    assert!(t.has_any());
    assert!(t.left.is_some());
    assert!(t.background.is_some());
    assert!(t.right.is_none());
    assert!(t.foreground.is_none());

    let left = t.left.unwrap();
    assert_eq!(left.duration, Duration::from_millis(100));
    assert_eq!(left.easing, Easing::Linear);
}

#[test]
fn test_transitions_position_group() {
    let t = Transitions::new().position(Duration::from_millis(300), Easing::EaseOut);

    assert!(t.left.is_some());
    assert!(t.top.is_some());
    assert!(t.right.is_some());
    assert!(t.bottom.is_some());
    assert!(t.width.is_none());
    assert!(t.background.is_none());
}

#[test]
fn test_transitions_size_group() {
    let t = Transitions::new().size(Duration::from_millis(200), Easing::EaseIn);

    assert!(t.width.is_some());
    assert!(t.height.is_some());
    assert!(t.left.is_none());
}

#[test]
fn test_transitions_colors_group() {
    let t = Transitions::new().colors(Duration::from_millis(500), Easing::EaseInOut);

    assert!(t.background.is_some());
    assert!(t.foreground.is_some());
    assert!(t.left.is_none());
}

#[test]
fn test_transitions_all_group() {
    let t = Transitions::new().all(Duration::from_millis(400), Easing::Linear);

    assert!(t.left.is_some());
    assert!(t.top.is_some());
    assert!(t.right.is_some());
    assert!(t.bottom.is_some());
    assert!(t.width.is_some());
    assert!(t.height.is_some());
    assert!(t.background.is_some());
    assert!(t.foreground.is_some());
}

#[test]
fn test_transitions_clone() {
    let t1 = Transitions::new().background(Duration::from_millis(300), Easing::EaseOut);
    let t2 = t1.clone();

    assert!(t2.background.is_some());
    assert_eq!(t2.background.unwrap().duration, Duration::from_millis(300));
}

// =============================================================================
// AnimationState Tests
// =============================================================================

#[test]
fn test_animation_state_new() {
    let state = AnimationState::new();
    assert!(!state.has_active_transitions());
}

#[test]
fn test_animation_state_no_transitions_without_config() {
    let mut state = AnimationState::new();

    // Element without transitions configured
    let element = Element::text("test").id("test");

    state.update(&element);
    assert!(!state.has_active_transitions());
}

#[test]
fn test_animation_state_no_transition_on_first_frame() {
    let mut state = AnimationState::new();

    // Element with transitions, but first frame (no previous value)
    let element = Element::text("test")
        .id("test")
        .style(Style::new().background(Color::oklch(0.5, 0.1, 200.0)))
        .transitions(Transitions::new().background(Duration::from_millis(300), Easing::Linear));

    state.update(&element);
    // First frame just captures snapshot, no transition
    assert!(!state.has_active_transitions());
}

#[test]
fn test_animation_state_transition_on_change() {
    let mut state = AnimationState::new();

    // First frame - capture initial state
    let element1 = Element::text("test")
        .id("test")
        .style(Style::new().background(Color::oklch(0.5, 0.1, 200.0)))
        .transitions(Transitions::new().background(Duration::from_millis(300), Easing::Linear));

    state.update(&element1);
    assert!(!state.has_active_transitions());

    // Second frame - change the background color
    let element2 = Element::text("test")
        .id("test")
        .style(Style::new().background(Color::oklch(0.8, 0.2, 100.0)))
        .transitions(Transitions::new().background(Duration::from_millis(300), Easing::Linear));

    state.update(&element2);
    // Should now have an active transition
    assert!(state.has_active_transitions());
}

#[test]
fn test_animation_state_no_transition_without_change() {
    let mut state = AnimationState::new();

    let element = Element::text("test")
        .id("test")
        .style(Style::new().background(Color::oklch(0.5, 0.1, 200.0)))
        .transitions(Transitions::new().background(Duration::from_millis(300), Easing::Linear));

    // First frame
    state.update(&element);
    // Second frame with same value
    state.update(&element);

    // No change, no transition
    assert!(!state.has_active_transitions());
}

#[test]
fn test_animation_state_reduced_motion() {
    let mut state = AnimationState::new();
    state.set_reduced_motion(true);

    // First frame
    let element1 = Element::text("test")
        .id("test")
        .style(Style::new().background(Color::oklch(0.5, 0.1, 200.0)))
        .transitions(Transitions::new().background(Duration::from_millis(300), Easing::Linear));

    state.update(&element1);

    // Second frame with change
    let element2 = Element::text("test")
        .id("test")
        .style(Style::new().background(Color::oklch(0.8, 0.2, 100.0)))
        .transitions(Transitions::new().background(Duration::from_millis(300), Easing::Linear));

    state.update(&element2);

    // With reduced motion, transitions are skipped
    assert!(!state.has_active_transitions());
}

#[test]
fn test_animation_state_cleanup_removes_old_elements() {
    let mut state = AnimationState::new();

    // First frame with element
    let element1 = Element::text("test")
        .id("elem1")
        .style(Style::new().background(Color::oklch(0.5, 0.1, 200.0)))
        .transitions(Transitions::new().background(Duration::from_millis(300), Easing::Linear));

    state.update(&element1);

    // Second frame - trigger transition
    let element2 = Element::text("test")
        .id("elem1")
        .style(Style::new().background(Color::oklch(0.8, 0.2, 100.0)))
        .transitions(Transitions::new().background(Duration::from_millis(300), Easing::Linear));

    state.update(&element2);
    assert!(state.has_active_transitions());

    // Cleanup with empty set (element removed)
    let empty_ids: HashSet<String> = HashSet::new();
    state.cleanup(&empty_ids);

    // Transitions for removed element should be gone
    assert!(!state.has_active_transitions());
}

#[test]
fn test_animation_state_get_interpolated() {
    let mut state = AnimationState::new();

    // First frame
    let element1 = Element::text("test")
        .id("test")
        .style(Style::new().background(Color::oklch(0.5, 0.1, 200.0)))
        .transitions(Transitions::new().background(Duration::from_millis(300), Easing::Linear));

    state.update(&element1);

    // Second frame - trigger transition
    let element2 = Element::text("test")
        .id("test")
        .style(Style::new().background(Color::oklch(0.8, 0.2, 100.0)))
        .transitions(Transitions::new().background(Duration::from_millis(300), Easing::Linear));

    state.update(&element2);

    // Should be able to get interpolated value
    let interpolated = state.get_interpolated("test", TransitionProperty::Background);
    assert!(interpolated.is_some());

    // Should be a color
    match interpolated.unwrap() {
        PropertyValue::Color(_) => {}
        _ => panic!("Expected Color property value"),
    }
}

#[test]
fn test_animation_state_no_interpolated_without_transition() {
    let mut state = AnimationState::new();

    let element = Element::text("test").id("test");
    state.update(&element);

    // No transition configured, should return None
    let interpolated = state.get_interpolated("test", TransitionProperty::Background);
    assert!(interpolated.is_none());
}

// =============================================================================
// collect_element_ids Tests
// =============================================================================

#[test]
fn test_collect_element_ids_single() {
    let element = Element::text("test").id("my-element");
    let ids = collect_element_ids(&element);

    assert!(ids.contains("my-element"));
    assert_eq!(ids.len(), 1);
}

#[test]
fn test_collect_element_ids_nested() {
    let element = Element::col()
        .id("parent")
        .child(Element::text("a").id("child1"))
        .child(Element::text("b").id("child2"));

    let ids = collect_element_ids(&element);

    assert!(ids.contains("parent"));
    assert!(ids.contains("child1"));
    assert!(ids.contains("child2"));
    assert_eq!(ids.len(), 3);
}

#[test]
fn test_collect_element_ids_deeply_nested() {
    let element = Element::col().id("root").child(
        Element::row()
            .id("row")
            .child(Element::text("deep").id("deep")),
    );

    let ids = collect_element_ids(&element);

    assert!(ids.contains("root"));
    assert!(ids.contains("row"));
    assert!(ids.contains("deep"));
    assert_eq!(ids.len(), 3);
}

// =============================================================================
// Element Integration Tests
// =============================================================================

#[test]
fn test_element_transitions_builder() {
    let element = Element::text("button")
        .id("btn")
        .transitions(Transitions::new().background(Duration::from_millis(200), Easing::EaseOut));

    assert!(element.transitions.background.is_some());
}

#[test]
fn test_element_transitions_default() {
    let element = Element::text("test");
    assert!(!element.transitions.has_any());
}
