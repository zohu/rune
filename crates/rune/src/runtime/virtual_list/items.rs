//! Retained application-selected windows. Selection is independent of scene size.
use std::rc::Rc;

use super::*;
use crate::VirtualItems;

#[derive(Default)]
pub(crate) struct Windows {
    pub(crate) lists: HashMap<String, Window>,
    pub(in crate::runtime) frame: u64,
}

pub(crate) struct Window {
    pub(crate) source: Rc<dyn VirtualItems>,
    revision: Option<u64>,
    keys: Vec<String>,
    shown: Vec<String>,
    present: bool,
    pinned: Option<String>,
    fill: Fill,
    admission: Admission,
}

enum Fill {
    Ready,
    Pending,
}

enum Admission {
    Idle,
    Frame { id: u64, budget: usize, keys: HashSet<String> },
}

impl Windows {
    pub(in crate::runtime) fn frame_begins(&mut self) {
        self.frame = self.frame.wrapping_add(1);
    }

    pub(in crate::runtime) fn begin_measure(&mut self) {
        for window in self.lists.values_mut() {
            window.present = false;
        }
    }
}

impl Window {
    pub(crate) fn new(source: Rc<dyn VirtualItems>) -> Self {
        Self {
            source,
            revision: None,
            keys: Vec::new(),
            shown: Vec::new(),
            present: true,
            pinned: None,
            fill: Fill::Ready,
            admission: Admission::Idle,
        }
    }

    pub(crate) fn changed(&self, path: &str, focused: Option<&str>) -> bool {
        self.revision != Some(self.source.version())
            || matches!(self.fill, Fill::Pending)
            || self.pinned.as_ref().is_some_and(|key| focused_key(path, focused).as_deref() != Some(key))
    }

    pub(crate) fn select(&mut self, path: &str, focused: Option<&str>, frame: u64) -> Vec<String> {
        let revision = self.source.version();
        if self.revision != Some(revision) {
            self.keys = self.source.keys();
            assert!(self.keys.iter().all(|k| !k.contains('/')), "virtual item keys cannot contain /");
            let mut seen = HashSet::default();
            assert!(self.keys.iter().all(|k| seen.insert(k)), "virtual item keys must be unique");
            self.revision = Some(revision);
        }
        let shown: HashSet<_> = self.shown.iter().map(String::as_str).collect();
        let mut entering: Vec<_> = self
            .keys
            .iter()
            .enumerate()
            .filter(|(_, k)| !shown.contains(k.as_str()))
            .map(|(i, k)| (self.source.mount_priority(k), i))
            .collect();
        entering.sort_unstable();
        if !matches!(self.admission, Admission::Frame { id, .. } if id == frame) {
            self.admission = Admission::Frame {
                id: frame,
                budget: self.source.mount_budget().max(1),
                keys: HashSet::default(),
            };
        }
        let Admission::Frame { budget, keys: admitted, .. } = &mut self.admission else { unreachable!() };
        let remaining = budget.saturating_sub(admitted.len());
        let new: Vec<_> = entering
            .iter()
            .filter(|(_, i)| !admitted.contains(&self.keys[*i]))
            .take(remaining)
            .map(|(_, i)| self.keys[*i].clone())
            .collect();
        admitted.extend(new);
        self.fill = if entering.iter().any(|(_, i)| !admitted.contains(&self.keys[*i])) {
            Fill::Pending
        } else {
            Fill::Ready
        };
        let mut selected: Vec<_> = self
            .keys
            .iter()
            .filter(|k| shown.contains(k.as_str()) || admitted.contains(*k))
            .cloned()
            .collect();
        self.pinned = focused_key(path, focused)
            .filter(|key| shown.contains(key.as_ref()) && !selected.iter().any(|k| k == key.as_ref()))
            .map(std::borrow::Cow::into_owned);
        selected.extend(self.pinned.iter().cloned());
        selected
    }

    pub(in crate::runtime) fn cull_departed(&self, path: &str, keys: &[String], culled: &mut HashSet<String>) {
        let next: HashSet<_> = keys.iter().map(String::as_str).collect();
        culled.extend(self.shown.iter().filter(|k| !next.contains(k.as_str())).map(|k| child_path(path, k)));
    }
}

fn focused_key<'a>(path: &str, focused: Option<&'a str>) -> Option<std::borrow::Cow<'a, str>> {
    let child = focused?.strip_prefix(path)?.strip_prefix('/')?.split('/').next()?;
    element_path::decode_key_segment(child)
}

impl RuntimeCore {
    pub(in crate::runtime) fn applied_items(&mut self, present_len: usize) {
        if self.commit.items.lists.is_empty() { return; }
        let Self { commit, tree, .. } = self;
        commit.items.lists.retain(|path, v| {
            let Some(i) = tree.path_ids.get(path).and_then(|id| tree.node_index.get(id)).copied() else { return false };
            if !v.present || i >= present_len || tree.nodes[i].path != *path { return false; }
            v.shown.clear();
            v.shown.extend(tree.nodes[i + 1..present_len]
                .iter()
                .take_while(|n| n.parent.is_some_and(|p| p >= i))
                .filter(|n| n.parent == Some(i))
                .filter_map(|n| n.key.clone()));
            true
        });
    }

    pub(in crate::runtime) fn item_window_changed(&self) -> bool {
        self.commit.items.lists.iter().any(|(path, v)| v.changed(path, self.kbd.focus.path()))
    }
}

impl Measurement<'_> {
    pub(in crate::runtime) fn populate_items(&mut self, el: &mut El, path: &str) {
        let Some(source) = el.cold.get_mut_if_some().and_then(|c| c.virtual_items.take()) else { return };
        assert!(
            el.children.is_empty() && el.cold.virtual_estimate.is_none(),
            "virtual_items owns its children"
        );
        let v = self.items.lists.entry(path.to_owned()).or_insert_with(|| Window::new(source.clone()));
        if !Rc::ptr_eq(&v.source, &source) {
            v.revision = None;
        }
        v.source = source;
        v.present = true;
        let keys = v.select(path, self.focused, self.items.frame);
        v.cull_departed(path, &keys, self.virtual_culled);
        el.children = keys.iter().map(|key| build_item(&*v.source, path, key, self.key_warnings)).collect();
    }
}

pub(in crate::runtime) fn build_item(
    source: &dyn VirtualItems, path: &str, key: &str, key_warnings: &mut Option<Vec<String>>,
) -> El {
    let tag = crate::memo::virtual_row_tag(path, key);
    let mut item = crate::memo::in_virtual_row(tag, || source.build(key)).key(key.to_owned());
    item.style.set_absolute_inset(item.style.inset());
    item.flags.set(crate::el::ElFlags::VIRTUAL_MOUNT, true);
    if !crate::input::key_diagnostics::clean(&item) {
        let duplicates = crate::input::key_diagnostics::repair_at(&mut item, &child_path(path, key));
        match key_warnings.as_mut() {
            Some(out) => out.extend(duplicates.into_iter().map(|d| d.message)),
            None => crate::input::key_diagnostics::warn_once(&duplicates),
        }
    }
    item
}
