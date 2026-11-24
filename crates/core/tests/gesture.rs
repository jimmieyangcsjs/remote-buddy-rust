use remote_buddy_core::input::{ButtonPhase, Gesture, GestureRecognizer, RemoteButton};

#[test]
fn click_is_committed_after_double_click_window() {
    let mut recognizer = GestureRecognizer::new();
    assert!(
        recognizer
            .on_event(RemoteButton::Ok, ButtonPhase::Down, 0, true)
            .is_empty()
    );
    assert!(
        recognizer
            .on_event(RemoteButton::Ok, ButtonPhase::Up, 100, true)
            .is_empty()
    );
    assert_eq!(recognizer.poll(449), Vec::<Gesture>::new());
    assert_eq!(recognizer.poll(450), vec![Gesture::Click]);
}

#[test]
fn second_click_becomes_double_click() {
    let mut recognizer = GestureRecognizer::new();
    recognizer.on_event(RemoteButton::Ok, ButtonPhase::Down, 0, true);
    recognizer.on_event(RemoteButton::Ok, ButtonPhase::Up, 100, true);
    recognizer.on_event(RemoteButton::Ok, ButtonPhase::Down, 200, true);
    assert_eq!(
        recognizer.on_event(RemoteButton::Ok, ButtonPhase::Up, 300, true),
        vec![Gesture::DoubleClick]
    );
    assert!(recognizer.poll(1000).is_empty());
}

#[test]
fn long_press_wins_over_click() {
    let mut recognizer = GestureRecognizer::new();
    recognizer.on_event(RemoteButton::Ok, ButtonPhase::Down, 0, true);
    assert_eq!(
        recognizer.on_event(RemoteButton::Ok, ButtonPhase::Up, 700, true),
        vec![Gesture::LongPress]
    );
}

#[test]
fn repeat_is_rate_limited() {
    let mut recognizer = GestureRecognizer::new();
    recognizer.on_event(RemoteButton::Up, ButtonPhase::Down, 0, true);
    assert!(
        recognizer
            .on_event(RemoteButton::Up, ButtonPhase::Repeat, 500, true)
            .is_empty()
    );
    assert_eq!(
        recognizer.on_event(RemoteButton::Up, ButtonPhase::Repeat, 700, true),
        vec![Gesture::Repeat]
    );
}

#[test]
fn click_is_immediate_when_double_click_is_disabled() {
    let mut recognizer = GestureRecognizer::new();
    recognizer.on_event(RemoteButton::Ok, ButtonPhase::Down, 0, false);
    assert_eq!(
        recognizer.on_event(RemoteButton::Ok, ButtonPhase::Up, 100, false),
        vec![Gesture::Click]
    );
    assert!(recognizer.poll(1000).is_empty());
}
