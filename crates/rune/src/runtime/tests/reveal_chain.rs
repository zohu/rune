//! Streamed words fade in reading order across texts: a chunk that spans several paragraphs or
//! list items fades them one after another, so the fading edge is only ever on the newest words.
use super::*;

#[derive(Default)]
struct Doc {
    paras: Vec<String>,
    point: Option<crate::Live<[f32; 2]>>,
}

fn view(s: &Doc, _: &mut Cx<Doc>) -> El {
    col().w_full().children(s.paras.iter().enumerate().map(|(i, p)| crate::text(p.clone()).wrap().w_full().reveal(0.3).key(format!("p{i}"))))
}

fn words(n: usize) -> String {
    (0..n).map(|i| format!("word{i}")).collect::<Vec<_>>().join(" ")
}

/// Each paragraph's (first, last) word start time.
fn times(rt: &Runtime<Doc>, key: &str) -> Option<(f64, f64)> {
    let id = rt.tree.nodes.iter().find(|n| n.key.as_deref() == Some(key))?.id;
    let w = rt.motion.reveals_map().get(&id)?.words();
    Some((w.first()?.1, w.last()?.1))
}

fn runtime() -> Runtime<Doc> {
    let mut rt = Runtime::new(Doc { paras: vec![String::new()], ..Doc::default() }, view, (600.0, 800.0), Env::new(true, 2.0));
    rt.frame(0.0);
    rt.frame(1.0 / 60.0);
    rt
}

#[test]
fn one_chunk_over_three_paragraphs_fades_them_in_order() {
    let mut rt = runtime();
    rt.state.paras = vec![words(12), words(12), words(12)];
    rt.invalidate();
    rt.frame(1.0 / 60.0);
    let [a, b, c] = ["p0", "p1", "p2"].map(|k| times(&rt, k).unwrap_or_else(|| panic!("{k} reveals")));
    assert!(b.0 > a.1 && c.0 > b.1, "each paragraph starts after the one above ends: {a:?} {b:?} {c:?}");
}

#[test]
fn a_new_paragraph_waits_for_the_last_ones_tail() {
    let mut rt = runtime();
    rt.state.paras = vec![words(30)];
    rt.invalidate();
    rt.frame(1.0 / 60.0);
    let a = times(&rt, "p0").unwrap();
    rt.state.paras.push(words(4));
    rt.invalidate();
    rt.frame(1.0 / 60.0);
    let b = times(&rt, "p1").unwrap();
    assert!(b.0 > a.1, "the next paragraph starts after the last word above: {a:?} then {b:?}");
}

#[test]
fn once_everything_has_shown_a_new_paragraph_starts_at_once() {
    let mut rt = runtime();
    rt.state.paras = vec![words(8)];
    rt.invalidate();
    rt.frame(1.0 / 60.0);
    for _ in 0..120 {
        rt.frame(1.0 / 60.0);
    }
    rt.state.paras.push(words(4));
    rt.invalidate();
    rt.frame(1.0 / 60.0);
    let (first, _) = times(&rt, "p1").unwrap();
    assert!((first - rt.t).abs() < 0.02, "no wait once nothing is fading: starts {first}, now {}", rt.t);
}

/// Texts mounting together behind a run longer than a fade (a remount, a restored turn) fade
/// together, as before, rather than one after another through still frames.
#[test]
fn texts_behind_a_long_run_fade_together() {
    let mut rt = runtime();
    rt.state.paras = vec![words(120), words(120), words(120)];
    rt.invalidate();
    rt.frame(1.0 / 60.0);
    let [a, b, c] = ["p0", "p1", "p2"].map(|k| times(&rt, k).unwrap_or_else(|| panic!("{k} reveals")));
    assert!(a.1 - a.0 > 0.3, "the first paragraph's run outlasts a fade: {a:?}");
    assert!(b.0 < a.1 && c.0 < a.1, "the rest don't wait for it: {a:?} {b:?} {c:?}");
}

#[test]
fn paced_reveal_follows_display_units_and_bounds_a_burst() {
    let point = crate::Live::new([0.0; 2]);
    let mut rt = Runtime::new(Doc { point: Some(point.clone()), ..Doc::default() }, |s: &Doc, _: &mut Cx<Doc>| {
        col().w(540.0).children(s.paras.iter().enumerate().map(|(i, p)|
            crate::text(p).key(format!("p{i}")).w_full().wrap().reveal(0.0)))
            .paced_reveal(0.16, 0.03, 0.28, s.point.as_ref())
    }, (600.0, 600.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.state.paras = vec!["海风Hello👩‍💻".into()];
    rt.invalidate();
    rt.frame(0.016);
    let id = rt.tree.nodes.iter().find(|n| n.key.as_deref() == Some("p0")).unwrap().id;
    let words = rt.motion.reveals_map()[&id].words();
    assert_eq!(words.iter().map(|w| w.0).collect::<Vec<_>>(), [0, 3, 6, 11]);
    let start = point.get();
    for _ in 0..9 { rt.frame(1.0 / 60.0); }
    assert!(point.get()[0] > start[0], "the shaped caret advances between appends");

    // The ZWJ and new glyph extend the last grapheme. No independent fade may split it.
    rt.state.paras[0].push_str("\u{200d}🚀");
    rt.state.paras.push("海风".repeat(800));
    rt.invalidate();
    rt.frame(0.016);
    let words = rt.motion.reveals_map()[&id].words();
    assert!(words.iter().all(|w| w.0 <= 11));
    let (first, last) = times(&rt, "p1").unwrap();
    assert!(last > first && last - rt.t <= 0.281, "the whole burst has a bounded reading edge");
    for _ in 0..32 { rt.frame(1.0 / 60.0); }
    assert!(times(&rt, "p1").is_none());
}

#[test]
fn paced_code_uses_line_boundaries_and_motion_levels_land_the_reading_edge() {
    let mut rt = Runtime::new(Doc::default(), |s, _| {
        let value = s.paras.first().map_or("", String::as_str);
        col().children([("code", 0.16, 0.03), ("reduced", 0.12, 0.0), ("instant", 0.0, 0.0)].map(|(key, fade, step)| {
            col().child(crate::text(value).key(key).by_line().reveal(0.0)).paced_reveal(fade, step, 0.28, None)
        }))
    }, (600.0, 400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.state.paras.push("first word\nsecond line\nthird".into());
    rt.invalidate();
    rt.frame(0.016);
    let id = rt.tree.nodes.iter().find(|n| n.key.as_deref() == Some("code")).unwrap().id;
    let words = rt.motion.reveals_map()[&id].words();
    assert_eq!(words.iter().map(|w| w.0).collect::<Vec<_>>(), [0, 11, 23]);
    assert!(words[0].1 < words[1].1 && words[1].1 < words[2].1);
    let (first, last) = times(&rt, "reduced").unwrap();
    assert_eq!(first, last);
    assert!(times(&rt, "instant").is_none());
}

#[test]
fn paced_reveal_motion_gate() {
    use crate::motion_gate::{Job, Run, gate_rows};
    gate_rows(vec![("stream/one-burst".into(), Box::new(|| {
        let mut rt = Runtime::new(Doc::default(), |s, cx| view(s, cx)
            .paced_reveal(0.16, 0.03, 0.28, None), (400.0, 140.0), Env::new(true, 1.0));
        rt.state.paras.push(String::new());
        let mut run = Run::new(rt, 120.0);
        run.idle(0.1);
        run.act("append", |rt| {
            rt.state.paras[0].push_str("海上生明月，天涯共此时。Hello 🌊");
            rt.invalidate();
        });
        run.play(0.55);
        run.quiet();
        run.play(0.15);
        run.rec
    }) as Job)], &[]);
}
