//! Retained application-selected windows. Selection is independent of scene size.
use std::rc::Rc;

use super::*;
use crate::VirtualItems;

#[derive(Default)]
pub(crate) struct Windows {
    pub(crate) lists: HashMap<String, Window>,
    pub(crate) frame: u64,
}

pub(crate) struct Window {
    pub(crate) source: Rc<dyn VirtualItems>,
    revision: Option<u64>,
    keys: Vec<String>,
    pub(crate) shown: Vec<String>,
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

impl Window {
    pub(crate) fn new(source: Rc<dyn VirtualItems>) -> Self {
        Self {
            source,
            revision: None,
            keys: Vec::new(),
            shown: Vec::new(),
            fill: Fill::Ready,
            admission: Admission::Idle,
        }
    }

    pub(crate) fn changed(&self, path: &str, focused: Option<&str>) -> bool {
        self.revision != Some(self.source.version())
            || matches!(self.fill, Fill::Pending)
            || self.shown.iter().any(|k| !self.keys.contains(k) && !is_focused(path, k, focused))
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
        let shown: HashSet<_> = self.shown.iter().collect();
        let mut entering: Vec<_> = self
            .keys
            .iter()
            .enumerate()
            .filter(|(_, k)| !shown.contains(k))
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
            .enumerate()
            .filter(|(_, k)| shown.contains(k) || admitted.contains(*k))
            .map(|(_, k)| k.clone())
            .collect();
        for key in &self.shown {
            if !selected.contains(key) && is_focused(path, key, focused) {
                selected.push(key.clone());
            }
        }
        selected
    }
}

fn is_focused(path: &str, key: &str, focused: Option<&str>) -> bool {
    focused
        .and_then(|f| f.strip_prefix(path))
        .and_then(|r| r.strip_prefix('/'))
        .is_some_and(|r| r == key || r.strip_prefix(key).is_some_and(|r| r.starts_with('/')))
}

impl<S: 'static> Runtime<S> {
    pub(super) fn applied_items(&mut self) {
        self.items.lists.retain(|path, v| {
            let Some(i) = self.nodes.iter().position(|n| n.path == *path) else { return false };
            v.shown =
                self.nodes.iter().filter(|n| n.parent == Some(i)).filter_map(|n| n.key.clone()).collect();
            true
        });
    }

    pub(super) fn item_window_changed(&self) -> bool {
        self.items.lists.iter().any(|(path, v)| v.changed(path, self.focused.as_deref()))
    }
}

impl<S: 'static> Measurement<'_, S> {
    pub(super) fn populate_items(&mut self, el: &mut El, path: &str) {
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
        let keys = v.select(path, self.focused, self.items.frame);
        for key in v.shown.iter().filter(|k| !keys.contains(k)) {
            self.virtual_culled.insert(format!("{path}/{key}"));
        }
        el.children = keys.iter().map(|key| build_item(&v.source, path, key)).collect();
    }
}

pub(super) fn build_item(source: &Rc<dyn VirtualItems>, path: &str, key: &str) -> El {
    let tag = crate::memo::virtual_row_tag(path, key);
    let mut item = crate::memo::in_virtual_row(tag, || source.build(key)).key(key.to_owned());
    item.style.set_absolute_inset(item.style.inset());
    item.flags.set(crate::el::ElFlags::VIRTUAL_MOUNT, true);
    let duplicates = crate::input::key_diagnostics::repair_at(&mut item, &format!("{path}/{key}"));
    crate::input::key_diagnostics::warn_once(&duplicates);
    item
}
