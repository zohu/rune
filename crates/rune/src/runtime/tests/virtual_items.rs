//! Application-selected windows use the retained remount path, including live text.
use super::*;
use crate::{Live, LiveText, VirtualItems, memo, text};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub(super) struct Source {
    pub(super) revision: Cell<u64>,
    pub(super) keys: RefCell<Vec<String>>,
    pub(super) builds: RefCell<Vec<String>>,
    pub(super) reads: Cell<usize>,
    pub(super) budget: Cell<usize>,
    pub(super) opacity: Live<f32>,
    pub(super) label: LiveText,
    pub(super) complex: bool,
    pub(super) nested: Cell<bool>,
    animated: bool,
}
impl Source {
    pub(super) fn new(complex: bool) -> Self {
        Self {
            revision: Cell::new(0),
            keys: RefCell::new(vec!["a".into(), "b".into()]),
            builds: RefCell::new(Vec::new()),
            reads: Cell::new(0),
            budget: Cell::new(usize::MAX),
            opacity: Live::new(1.0),
            label: LiveText::new("x"),
            complex,
            nested: Cell::new(false),
            animated: false,
        }
    }
    pub(super) fn window(&self, keys: Vec<String>) {
        *self.keys.borrow_mut() = keys;
        self.revision.set(self.revision.get() + 1);
    }
}
impl VirtualItems for Source {
    fn version(&self) -> u64 {
        self.revision.get()
    }
    fn keys(&self) -> Vec<String> {
        self.reads.set(self.reads.get() + 1);
        self.keys.borrow().clone()
    }
    fn mount_budget(&self) -> usize {
        self.budget.get()
    }
    fn mount_priority(&self, key: &str) -> u64 {
        u64::MAX - key.bytes().map(u64::from).sum::<u64>()
    }
    fn build(&self, key: &str) -> El {
        self.builds.borrow_mut().push(key.into());
        if self.nested.get() {
            return col().size(120.0, 40.0).scroll(crate::ScrollSpec { stick_bottom: false }).child(
                col().growth(crate::GrowthSpec::default()).virtual_rows(
                    20,
                    20.0,
                    |_: &App, i| format!("r{i}").into(),
                    |_: &App, _| el().h(20.0),
                ),
            );
        }
        memo(format!("item-{key}"), 0, || {
            let mut item = col()
                .absolute(20.0, 20.0)
                .size(120.0, 80.0)
                .live_opacity(&self.opacity)
                .child(text("x").key("label").focusable(true).live_text(&self.label));
            if self.complex {
                item = item.children(
                    (0..18).map(|i| el().key(format!("p{i}")).size(4.0, 2.0).live_opacity(&self.opacity)),
                );
            }
            if self.animated {
                item = item.child(
                    el().key("loop")
                        .size(10.0, 10.0)
                        .looping(rune_motion::LoopSpec::opacity(2.0, 0.0, 0.2, 1.0)),
                );
            }
            item
        })
    }
}
pub(super) struct App {
    pub(super) source: Rc<Source>,
    pub(super) views: Cell<usize>,
    pub(super) rows: bool,
}
pub(super) fn view(s: &App, _: &mut Cx<App>) -> El {
    s.views.set(s.views.get() + 1);
    let mut root = col().size(600.0, 400.0).children([
        text("x").key("outside").self_start().live_text(&s.source.label),
        el().key("items").size(300.0, 200.0).virtual_items(s.source.clone()),
    ]);
    if s.rows {
        root = root.child(
            col().key("scroll").size(200.0, 160.0).scroll(crate::ScrollSpec { stick_bottom: false }).child(
                col().key("rows").w_full().growth(crate::GrowthSpec::default()).virtual_rows(
                    100,
                    20.0,
                    |_: &App, i| format!("r{i}").into(),
                    |_: &App, _| el().h(20.0),
                ),
            ),
        );
    }
    root
}
pub(super) fn runtime(complex: bool, rows: bool) -> Runtime<App> {
    let mut rt = Runtime::new(
        App { source: Rc::new(Source::new(complex)), views: Cell::new(0), rows },
        view,
        (600.0, 400.0),
        Env::new(true, 2.0),
    );
    rt.set_live_visible(true);
    rt.frame(0.0);
    for _ in 0..20 {
        rt.frame(1.0 / 120.0);
    }
    rt
}
fn mounted(rt: &Runtime<App>) -> Vec<String> {
    rt.tree.nodes
        .iter()
        .filter(|n| n.parent.is_some_and(|p| rt.tree.nodes[p].path == "#0/items"))
        .filter_map(|n| n.key.clone())
        .collect()
}

#[test]
fn window_retains_nodes_bindings_memos_and_live_text_inside_and_outside() {
    let mut rt = runtime(false, false);
    let a = rt.tree.nodes.iter().find(|n| n.path == "#0/items/a").unwrap().id;
    let views = rt.state.views.get();
    let placeholder_width = rt.tree.nodes.iter().find(|n| n.path == "#0/outside").unwrap().local.w;
    rt.state.source.builds.borrow_mut().clear();
    rt.state.source.opacity.set(0.25);
    rt.state.source.label.set("A much longer live label");
    rt.state.source.window(vec!["c".into(), "a".into()]);
    rt.frame(1.0 / 120.0);
    assert_eq!(rt.state.views.get(), views);
    assert_eq!(&*rt.state.source.builds.borrow(), &["c"]);
    assert_eq!(rt.tree.nodes.iter().find(|n| n.path == "#0/items/a").map(|n| n.id), Some(a));
    assert_eq!(mounted(&rt), ["c", "a"]);
    assert_eq!(rt.scene.get(a, prop::OPACITY), Some(0.25));
    for path in ["#0/outside", "#0/items/a/label", "#0/items/c/label"] {
        let node = rt.tree.nodes.iter().find(|n| n.path == path).unwrap();
        assert_eq!(node.text.as_ref().unwrap().content, "A much longer live label");
        assert!(node.text_layout.as_ref().unwrap().view().full_width() > 100.0);
    }
    assert_eq!(rt.tree.nodes.iter().find(|n| n.path == "#0/outside").unwrap().local.w, placeholder_width);
    // Once the visible content differs from the authored placeholder, another remount still
    // measures the original width.
    rt.state.source.window(vec!["a".into(), "e".into()]);
    rt.frame(1.0 / 120.0);
    assert_eq!(rt.state.views.get(), views);
    assert_eq!(rt.tree.nodes.iter().find(|n| n.path == "#0/outside").unwrap().local.w, placeholder_width);
    rt.state.source.window(vec!["c".into(), "a".into()]);
    rt.invalidate();
    rt.frame(1.0 / 120.0);
    assert_eq!(rt.tree.nodes.iter().find(|n| n.path == "#0/items/a").map(|n| n.id), Some(a));
    assert_eq!(mounted(&rt), ["c", "a"]);
}

#[test]
fn budget_priority_and_stacking_are_independent_and_eventually_sleep() {
    let mut rt = runtime(false, false);
    rt.state.source.budget.set(1);
    rt.state.source.window(vec!["a".into(), "b".into(), "c".into(), "d".into(), "e".into()]);
    let views = rt.state.views.get();
    rt.frame(1.0 / 120.0);
    assert_eq!(mounted(&rt), ["a", "b", "e"]);
    rt.frame(1.0 / 120.0);
    assert_eq!(mounted(&rt), ["a", "b", "d", "e"]);
    rt.frame(1.0 / 120.0);
    assert_eq!(mounted(&rt), ["a", "b", "c", "d", "e"]);
    for _ in 0..20 {
        rt.frame(1.0 / 120.0);
    }
    assert!(!rt.frame(1.0 / 120.0));
    assert_eq!(rt.state.views.get(), views);
    assert_eq!(rt.state.source.reads.get(), 2);
}

#[test]
fn focus_pin_retires_when_focus_leaves_without_a_revision_bump() {
    let mut rt = runtime(false, false);
    rt.state.source.window(vec!["#0".into(), "b".into()]);
    rt.frame(1.0 / 120.0);
    let path = child_path("#0/items", "#0");
    let label = child_path(&path, "label");
    let id = rt.node(&path).unwrap().id;
    rt.kbd.focus.focus = crate::runtime::focus_machine::Focus::On { path: label.clone(), hovered: false, from: None };
    rt.state.source.window(vec!["c".into()]);
    let views = rt.state.views.get();
    rt.frame(1.0 / 120.0);
    assert_eq!(rt.node(&path).map(|n| n.id), Some(id));
    assert_eq!(rt.kbd.focus.path(), Some(label.as_str()));
    rt.kbd.focus.focus = Default::default();
    rt.frame(1.0 / 120.0);
    assert!(rt.node(&path).is_none());
    assert_eq!(mounted(&rt), ["c"]);
    assert_eq!(rt.state.views.get(), views);
}

#[test]
fn reusing_the_container_path_without_virtual_items_releases_the_source() {
    let source = Rc::new(Source::new(false));
    let weak = Rc::downgrade(&source);
    let mut rt = Runtime::new(
        Some(source),
        |source, _| {
            let items = el().key("items").size(300.0, 200.0);
            if let Some(source) = source {
                items.virtual_items(source.clone())
            } else {
                items.child(el().key("authored").size(20.0, 20.0))
            }
        },
        (300.0, 200.0),
        Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt.update(|source, _| *source = None);
    rt.frame(0.0);
    assert!(rt.find("authored").is_some());
    assert!(weak.upgrade().is_none());
}

#[test]
fn root_commit_and_partial_commit_select_the_same_window_with_a_budget() {
    let mut partial = runtime(false, false);
    let mut full = runtime(false, false);
    for rt in [&mut partial, &mut full] {
        rt.state.source.budget.set(1);
        rt.state.source.window(vec!["b".into(), "c".into(), "d".into()]);
    }
    full.invalidate();
    partial.frame(1.0 / 120.0);
    full.frame(1.0 / 120.0);
    assert_eq!(mounted(&partial), mounted(&full));
    let rects = |rt: &Runtime<App>| rt.elements().into_iter().map(|n| (n.path, n.rect)).collect::<Vec<_>>();
    assert_eq!(rects(&partial), rects(&full));
    partial.frame(1.0 / 120.0);
    full.frame(1.0 / 120.0);
    assert_eq!(mounted(&partial), mounted(&full));
}

#[test]
fn indexed_rows_and_items_share_a_partial_commit() {
    let mut rt = runtime(false, true);
    let views = rt.state.views.get();
    rt.state.source.window(vec!["b".into(), "c".into()]);
    rt.frame(1.0 / 120.0);
    assert_eq!(rt.state.views.get(), views);
    assert_eq!(mounted(&rt), ["b", "c"]);
    assert!(rt.find("rows/r0").is_some());
}

#[test]
fn nested_virtual_items_fall_back_and_keep_the_selected_window() {
    let mut rt = runtime(false, false);
    rt.state.source.nested.set(true);
    rt.state.source.window(vec!["nested".into()]);
    let views = rt.state.views.get();
    rt.frame(1.0 / 120.0);
    assert!(rt.state.views.get() > views);
    assert_eq!(mounted(&rt), ["nested"]);
    assert!(rt.tree.nodes.iter().any(|n| n.key.as_deref() == Some("r0")));
}

#[test]
fn poll_publishes_the_window_before_the_same_frames_draw() {
    let mut rt = runtime(false, false);
    let views = rt.state.views.get();
    rt.poll = Some(|s, _, _| {
        s.source.window(vec!["poll".into()]);
        false
    });
    rt.frame(1.0 / 120.0);
    assert_eq!(mounted(&rt), ["poll"]);
    assert_eq!(rt.state.views.get(), views);
}

#[test]
fn resizing_and_changing_the_window_together_uses_the_current_window() {
    let mut rt = runtime(false, false);
    rt.state.source.window(vec!["resize".into()]);
    rt.set_viewport((700.0, 500.0), 2.0);
    rt.frame(1.0 / 120.0);
    assert_eq!(mounted(&rt), ["resize"]);
}

#[test]
fn first_mount_obeys_the_budget() {
    let source = Rc::new(Source::new(false));
    source.budget.set(1);
    let mut rt = Runtime::new(
        App { source, views: Cell::new(0), rows: false },
        view,
        (600.0, 400.0),
        Env::new(true, 2.0),
    );
    rt.frame(0.0);
    assert_eq!(mounted(&rt), ["b"]);
    rt.frame(1.0 / 120.0);
    assert_eq!(mounted(&rt), ["a", "b"]);
    assert_eq!(rt.state.views.get(), 1);
}

#[test]
fn repeated_commits_in_one_frame_share_the_mount_budget() {
    let mut rt = runtime(false, false);
    rt.state.source.budget.set(1);
    rt.state.source.window(vec!["a".into(), "c".into(), "d".into(), "e".into()]);
    rt.frame(1.0 / 120.0);
    assert_eq!(mounted(&rt), ["a", "e"]);
    rt.commit(&Tx::default());
    assert_eq!(mounted(&rt), ["a", "e"]);
    rt.state.source.window(vec!["a".into(), "z".into()]);
    rt.commit(&Tx::default());
    assert_eq!(mounted(&rt), ["a"]);
    rt.frame(1.0 / 120.0);
    assert_eq!(mounted(&rt), ["a", "z"]);
}

#[test]
fn retained_animation_keeps_its_clock_epoch() {
    let source = Rc::new(Source { animated: true, ..Source::new(false) });
    let mut rt = Runtime::new(
        App { source, views: Cell::new(0), rows: false },
        view,
        (600.0, 400.0),
        Env::new(true, 2.0),
    );
    rt.frame(0.0);
    rt.frame(0.5);
    let id = rt.tree.nodes.iter().find(|n| n.path == "#0/items/a/loop").unwrap().id;
    let track = *rt.motion.loop_track(id).unwrap();
    let views = rt.state.views.get();
    rt.state.source.window(vec!["a".into(), "c".into()]);
    rt.frame(0.125);
    assert_eq!(rt.motion.loop_track(id).unwrap().phase(rt.t), track.phase(rt.t));
    assert_eq!(rt.state.views.get(), views);
}
