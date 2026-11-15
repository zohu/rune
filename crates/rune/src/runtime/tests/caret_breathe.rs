//! `El::caret_breathe`: only the caret's alpha pulses, after a hold, while its window is in front.
use super::*;
use std::time::Duration;

const CARET: Color = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };

struct App {
    edit: crate::TextEdit,
    still: bool,
}

fn view(s: &App, cx: &mut Cx<App>) -> El {
    cx.reduced_motion(s.still);
    crate::text_input(&s.edit, crate::TextStyle::new(16.0, 400.0)).single_line()
        .caret(CARET, Color::WHITE.alpha(0.2))
        .caret_breathe(Duration::from_secs(1), 0.25, Duration::from_millis(500))
        .key("field").size(200.0, 40.0).on_edit(|s: &mut App| &mut s.edit)
}

fn runtime(still: bool) -> Runtime<App> {
    let mut edit = crate::TextEdit::new("breathe");
    edit.focused = true;
    let mut rt = Runtime::new(App { edit, still }, view, (200.0, 40.0), Env::new(true, 1.0));
    rt.set_live_visible(true);
    rt.window_focus(true);
    rt.frame(0.0);
    rt
}

/// The caret's drawn alpha: the one opaque-red rect of the frame.
fn alpha(rt: &Runtime<App>) -> f32 {
    let carets: Vec<_> = rt.instances.iter()
        .filter(|i| i.params[1] == KIND_RECT && i.fill[1] == 0.0 && i.fill[2] == 0.0 && i.rect[2] <= 3.0 && i.rect[3] > 8.0)
        .collect();
    assert_eq!(carets.len(), 1, "one caret");
    carets[0].fill[3]
}

#[test]
fn the_caret_holds_then_pulses_down_to_its_minimum_and_back() {
    let mut rt = runtime(false);
    assert_eq!(alpha(&rt), 1.0);
    // Inside the hold it stays whole, and one wake waits for the hold's end instead of frames.
    assert!(!rt.frame(0.25), "the hold draws no frames");
    assert_eq!(alpha(&rt), 1.0);
    // Half a period past the hold is the trough; a whole one is back at full.
    rt.frame(0.25);
    assert!(rt.frame(0.5), "the pulse asks for frames");
    assert!((alpha(&rt) - 0.25).abs() < 1e-3, "{}", alpha(&rt));
    rt.frame(0.5);
    assert!((alpha(&rt) - 1.0).abs() < 1e-3, "{}", alpha(&rt));
}

#[test]
fn a_key_restarts_the_hold() {
    let mut rt = runtime(false);
    rt.frame(0.5);
    rt.frame(0.5);
    assert!(alpha(&rt) < 0.3);
    rt.key(KeyEvent { key: crate::Key::Text("x".into()), mods: Default::default() });
    rt.frame(0.0);
    assert_eq!(rt.state.edit.text, "breathex");
    assert_eq!(alpha(&rt), 1.0, "typing shows the caret whole");
    rt.frame(0.25);
    assert_eq!(alpha(&rt), 1.0, "and holds it");
}

#[test]
fn reduced_motion_keeps_it_steady_and_a_window_behind_hides_it() {
    let mut rt = runtime(true);
    rt.frame(0.5);
    rt.frame(0.5);
    assert_eq!(alpha(&rt), 1.0, "reduced motion doesn't pulse");
    assert!(!rt.frame(0.1), "nor ask for frames");

    let mut rt = runtime(false);
    rt.window_focus(false);
    rt.frame(0.0);
    assert_eq!(alpha(&rt), 0.0, "a window behind draws no caret");
    rt.window_focus(true);
    rt.frame(0.0);
    assert_eq!(alpha(&rt), 1.0, "and it comes back whole, holding again");
}
