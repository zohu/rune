//! Host state changes must not hide behind a virtual window update.
use super::virtual_items::{App as ItemsApp, Source, view};
use super::*;
use crate::{App, AppSpec, Playground, Stage, Story};
use std::cell::Cell;
use std::rc::Rc;

fn items() -> ItemsApp {
    ItemsApp {
        source: Rc::new(Source::new(false)),
        views: Cell::new(0),
        rows: true,
    }
}

fn playground() -> Runtime<()> {
    let story = Story::new("Windows/Items", items, view)
        .stage(Stage::Fill)
        .poll(|s, _, _| {
            if s.source.revision.get() != 9 {
                return false;
            }
            s.rows = false;
            true
        })
        .side(|_, _| {
            el().key("side-change")
                .size(80.0, 30.0)
                .on_press(|s: &mut ItemsApp| s.rows = !s.rows)
        });
    let mut rt = App::new(AppSpec::new("Windows", (1000.0, 600.0), || (), |_, _| el()))
        .playground(Playground::new().enabled(true).story(story))
        .headless_runtime((1000.0, 600.0), Env::new(true, 1.0));
    rt.open_playground("Windows/Items", None);
    for _ in 0..10 {
        rt.frame(0.1);
    }
    rt
}

fn views(rt: &Runtime<()>) -> usize {
    rt.playground_state::<ItemsApp>().unwrap().views.get()
}
fn shift(rt: &Runtime<()>) {
    rt.playground_state::<ItemsApp>()
        .unwrap()
        .source
        .window(vec!["b".into(), "c".into()]);
}
fn tap(rt: &mut Runtime<()>, key: &str) {
    let (x, y) = rt.find(key).unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1.0 / 120.0);
}

#[test]
fn playground_virtual_windows_keep_the_story_and_chrome() {
    let mut rt = playground();
    let before = views(&rt);
    let id = rt
        .tree
        .nodes
        .iter()
        .find(|n| n.key.as_deref() == Some("b"))
        .unwrap()
        .id;
    shift(&rt);
    rt.frame(1.0 / 120.0);
    assert_eq!(views(&rt), before);
    assert_eq!(
        rt.tree.nodes
            .iter()
            .find(|n| n.key.as_deref() == Some("b"))
            .unwrap()
            .id,
        id
    );
    assert!(rt.playground_element("c").is_some());
    assert!(rt.find("playground-theme").is_some());
    // The indexed rows on the same stage also remount without the root.
    let scroll = rt.playground_element("scroll").unwrap().rect;
    rt.wheel(scroll[0] + 10.0, scroll[1] + 50.0, -400.0);
    for _ in 0..30 {
        rt.frame(1.0 / 120.0);
    }
    assert!(rt.commit.virtuals.values().next().unwrap().range.start > 0);
    assert_eq!(views(&rt), before);
}

#[test]
fn playground_requests_invalidate_even_an_unchanged_measurement() {
    let mut rt = playground();
    let before = views(&rt);
    // Exercise the commit boundary itself, including requests queued outside a handler.
    rt.hooks
        .playground
        .as_deref_mut()
        .unwrap()
        .driver_mut()
        .request(vec![crate::playground::Request::Dark(false)]);
    rt.commit.set_state_unchanged(true);
    rt.commit(&Tx::default());
    assert!(views(&rt) > before);
    assert!(rt.playground_element("a").is_some());
}

#[test]
fn playground_controls_and_story_side_changes_rebuild_the_stage() {
    let mut rt = playground();
    for key in ["playground-theme", "side-change"] {
        let before = views(&rt);
        shift(&rt);
        tap(&mut rt, key);
        assert!(views(&rt) > before, "{key} must rebuild the story");
    }
    assert!(!rt.playground_state::<ItemsApp>().unwrap().rows);
}

#[test]
fn playground_story_poll_changes_rebuild_the_stage() {
    let mut rt = playground();
    let before = views(&rt);
    rt.playground_state::<ItemsApp>()
        .unwrap()
        .source
        .revision
        .set(9);
    rt.frame(1.0 / 120.0);
    assert!(views(&rt) > before);
    assert!(rt.playground_element("rows").is_none());
}

#[test]
fn playground_focus_region_changes_at_commit_rebuild_the_chrome() {
    let mut rt = playground();
    let before = views(&rt);
    rt.kbd.focus.focus = crate::runtime::focus_machine::Focus::On { path: rt.playground_element("label").unwrap().path, hovered: false, from: None };
    rt.commit.set_state_unchanged(true);
    rt.commit(&Tx::default());
    assert!(views(&rt) > before);
    assert!(rt.find("playground-stage-ring").is_some());
}

#[test]
fn playground_navigation_replaces_the_stage_on_a_window_change() {
    let mut rt = playground();
    let old = rt.playground_element("a").unwrap().path;
    shift(&rt);
    rt.update(|_, tx| tx.playground.push(crate::playground::Request::Replay));
    rt.frame(1.0 / 120.0);
    assert_ne!(rt.playground_element("a").unwrap().path, old);
    assert!(rt.playground_element("c").is_none());
}
