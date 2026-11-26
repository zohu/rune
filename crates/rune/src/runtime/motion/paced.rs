//! One layout pass schedules the changed display runs of each document. Paint consumes normal
//! reveal phases; caret stops come from the same byte boundaries in those same shaped runs.

use std::ops::Range;
use crate::Live;
use unicode_segmentation::UnicodeSegmentation;

use super::*;
use super::reveal_phase::RevealPhase;

struct Scope {
    root: usize,
    end: usize,
    texts: Vec<usize>,
}

pub(super) struct CaretTrack {
    root: NodeId,
    point: Live<[f32; 2]>,
    stops: Vec<(f64, [f32; 2])>,
    next: usize,
}

impl CaretTrack {
    pub(super) fn advance(&mut self, scene: &Scene) -> bool {
        if !scene.contains(self.root) { return false; }
        self.sample(scene.store().node_time(self.root));
        self.next < self.stops.len()
    }

    fn sample(&mut self, now: f64) {
        let count = self.stops[self.next..].partition_point(|&(at, _)| at <= now);
        if count > 0 {
            self.next += count;
            self.point.set(self.stops[self.next - 1].1);
        }
    }
}

/// Include the preceding word in segmentation. A split network packet can extend a Latin
/// word, a combining sequence, a flag or a ZWJ emoji; its old glyphs keep their start time.
fn unit_starts(s: &str, from: usize, lines: bool, mut push: impl FnMut(usize)) {
    if from == s.len() { return; }
    if lines {
        if from == 0 || s.as_bytes()[from - 1] == b'\n' { push(from); }
        for (offset, _) in s[from..].match_indices('\n') {
            let next = from + offset + 1;
            if next < s.len() { push(next); }
        }
    } else {
        let context = s[..from].split_word_bound_indices().next_back().map_or(0, |(at, _)| at);
        for (offset, word) in s[context..].split_word_bound_indices() {
            let at = context + offset;
            if at >= from && !word.chars().all(char::is_whitespace) { push(at); }
        }
    }
}

impl RuntimeCore {
    /// Return the outer scopes so the legacy window-wide word chain skips their texts.
    pub(in crate::runtime) fn commit_paced_reveals(&mut self, nodes: &[Node], first: Option<usize>, land: bool) -> Vec<Range<usize>> {
        self.motion.paced_carets.clear();
        let mut outer = Vec::new();
        let Some(first) = first else { return outer };
        let mut scopes: Vec<Scope> = Vec::new();
        let mut active: Vec<usize> = Vec::new();
        for (i, node) in nodes.iter().enumerate().skip(first) {
            while let Some(&k) = active.last() {
                if node.parent.is_some_and(|p| p >= scopes[k].root) { break; }
                scopes[k].end = i;
                active.pop();
            }
            if node.vis.paced_reveal.is_some() {
                active.push(scopes.len());
                scopes.push(Scope { root: i, end: nodes.len(), texts: Vec::new() });
            }
            if let Some(&k) = active.last()
                && node.text.as_ref().is_some_and(|t| t.reveal >= 0.0)
            {
                scopes[k].texts.push(i);
            }
        }
        let mut finder = paint_tracks::Finder::default();
        for scope in scopes {
            if outer.last().is_none_or(|r: &Range<usize>| r.end <= scope.root) { outer.push(scope.root..scope.end); }
            self.schedule_scope(nodes, scope, land, &mut finder);
        }
        outer
    }

    fn schedule_scope(&mut self, nodes: &[Node], scope: Scope, land: bool, finder: &mut paint_tracks::Finder) {
        let root = &nodes[scope.root];
        let pace = root.vis.paced_reveal.as_ref().unwrap();
        let root_now = self.scene.store().node_time(root.id);
        // A zero interval already specifies the caller's simultaneous reduced-motion fade.
        let fade = if self.reduced_motion && pace.interval > 0.0 { pace.fade.min(0.15) } else { pace.fade };
        let step = if self.reduced_motion { 0.0 } else { f64::from(pace.interval) };
        let mut tail: Option<f64> = None;
        let mut additions = Vec::new();

        for &i in &scope.texts {
            let node = &nodes[i];
            let spec = node.text.as_ref().unwrap();
            let s = spec.content.as_str();
            let old = if self.tree.nodes.get(i).is_some_and(|old| old.id == node.id) {
                Some(i)
            } else {
                finder.find(&self.tree.nodes, node.id)
            };
            // A reused memo span leaves a moved stub in the previous tree.
            let old = paint_tracks::prev_node(&self.tree.nodes, nodes, old, i);
            let phase = self.motion.reveals.entry(node.id).or_default();
            let now = self.scene.store().node_time(node.id);
            let from = match old.and_then(|n| n.text.as_deref()) {
                Some(prev) if std::ptr::eq(prev, &**spec) || s.starts_with(&prev.content) => Some(prev.content.len()),
                Some(_) => None,
                None => Some(0),
            };
            let animate = !land && !node.virtual_mount && fade > 0.0;
            let Some(from) = from.filter(|&from| animate && from >= phase.shown()) else {
                *phase = RevealPhase::Idle { shown: s.len() };
                continue;
            };
            if let RevealPhase::Fading { words, fade: previous, .. } = phase {
                if *previous != fade || (step == 0.0 && words.last().is_some_and(|w| w.1 > now)) {
                    words.clear();
                } else {
                    let expired = words.partition_point(|w| w.1 + f64::from(fade) <= now);
                    words.drain(..expired);
                }
                if let Some(last) = words.last() { tail = Some(tail.map_or(last.1 - now, |t| t.max(last.1 - now))); }
                else { *phase = RevealPhase::Idle { shown: s.len() }; }
            }
            match phase {
                RevealPhase::Idle { shown } | RevealPhase::Fading { shown, .. } => *shown = s.len(),
            }
            unit_starts(s, from, spec.by_line, |at| additions.push((i, at as u32)));
        }

        // A large paste uses at most one start per 120 Hz frame within the delay budget.
        // Keep a small append's natural intervals even when the previous append is still fading.
        let limit = f64::from(pace.max_delay);
        let first = if step == 0.0 { 0.0 } else { tail.map_or(0.0, |last| (last + step).clamp(0.0, limit)) };
        let count = additions.len();
        let slots = count.min((((limit - first) * 120.0).ceil() as usize).saturating_add(1));
        let spacing = step.min((limit - first) / slots.saturating_sub(1).max(1) as f64);
        for (rank, (i, byte)) in additions.into_iter().enumerate() {
            let node = &nodes[i];
            let now = self.scene.store().node_time(node.id);
            let slot = rank.saturating_mul(slots.saturating_sub(1)) / count.saturating_sub(1).max(1);
            let delay = if slot == 0 && tail.is_none() && step > 0.0 { -0.25 * f64::from(fade) } else { first + spacing * slot as f64 };
            let phase = self.motion.reveals.get_mut(&node.id).unwrap();
            if let RevealPhase::Idle { shown } = *phase {
                *phase = RevealPhase::Fading { shown, words: Vec::new(), fade };
            }
            if let RevealPhase::Fading { words, .. } = phase {
                let at = now + delay;
                if words.last().is_none_or(|w| w.1 < at) { words.push((byte, at)); }
            }
        }

        if let Some(point) = &pace.caret {
            // With stagger, move when the next unit has ink. A simultaneous fade lands the caret.
            let threshold = if step > 0.0 { f64::from(fade) * 0.1 } else { 0.0 };
            self.schedule_caret(nodes, &scope, point, root_now, threshold);
        }
    }

    fn schedule_caret(&mut self, nodes: &[Node], scope: &Scope, point: &Live<[f32; 2]>, root_now: f64, threshold: f64) {
        let root = &nodes[scope.root];
        let mut stops = Vec::new();
        for &i in &scope.texts {
            let n = &nodes[i];
            let len = n.text.as_ref().unwrap().content.len();
            let now = self.scene.store().node_time(n.id);
            let words = match &self.motion.reveals[&n.id] {
                RevealPhase::Fading { words, .. } => &words[..],
                _ => &[],
            };
            let past = words.partition_point(|w| w.1 + threshold <= now);
            let visible = words.get(past).map_or(len, |w| w.0 as usize);
            if visible > 0 || (words.is_empty() && stops.is_empty()) {
                record_stop(&mut stops, root_now, i, visible);
            }
            for (k, &(_, at)) in words.iter().enumerate().skip(past) {
                let end = words.get(k + 1).map_or(len, |w| w.0 as usize);
                record_stop(&mut stops, root_now + (at - now) + threshold, i, end);
            }
        }
        let stops = stops.into_iter().filter_map(|(at, i, end)| {
            caret_point(&nodes[i], root, self.env.scale, end).map(|p| (at, p))
        }).collect();
        let mut track = CaretTrack { root: root.id, point: point.clone(), stops, next: 0 };
        if track.stops.is_empty() { point.set([0.0, (root.target[3] - root.target[1]) * 0.5]); }
        track.sample(root_now);
        if track.next < track.stops.len() { self.motion.paced_carets.push(track); }
    }
}

fn record_stop(stops: &mut Vec<(f64, usize, usize)>, at: f64, node: usize, byte: usize) {
    while stops.last().is_some_and(|s| s.0 >= at) { stops.pop(); }
    stops.push((at, node, byte));
}

fn caret_point(n: &Node, root: &Node, scale: f32, end: usize) -> Option<[f32; 2]> {
    let flow = n.text_layout.as_ref()?.view();
    let origin = text_origin(n, n.text.as_ref()?, flow, n.target, scale, 1.0);
    let byte = n.elided.map_or(end, |e| e.drawn(end, true));
    let rect = flow.caret(byte, Affinity::Upstream, 0.0);
    let x = origin[0] - root.target[0] + rect.x0 as f32 / scale;
    let right = (root.target[2] - root.target[0] - root.inset[2]).max(0.0);
    Some([x.clamp(0.0, right),
        origin[1] - root.target[1] + (rect.y0 + rect.y1) as f32 * 0.5 / scale])
}
