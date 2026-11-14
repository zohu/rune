//! Application-selected, absolutely positioned virtual windows.
use std::rc::Rc;

use crate::{El, el::ElFlags};

/// The UI-thread source of an application-selected virtual window.
///
/// Hold shared application data in the source (for example an `Rc<RefCell<_>>`), borrowing
/// only within each method. Rune retains the source until its container leaves the tree.
/// Publish changes and request a frame through the application's poll/input/wake mechanism;
/// this source does not wake an idle host by itself.
pub trait VirtualItems: 'static {
    /// Cheap revision read, checked after poll and input, before drawing each frame.
    /// Bump for changes to membership, stacking order or mount priority. To replace an
    /// already mounted item's content without a root commit, give it a new key.
    fn version(&self) -> u64;
    /// Unique stable keys in back-to-front stacking order, queried only when the revision
    /// changes. Return just the window's keys, not the entire scene. Keys must not contain `/`.
    fn keys(&self) -> Vec<String>;
    /// Build a keyed item's subtree. Rune sets its key and absolute positioning.
    fn build(&self, key: &str) -> El;
    /// Maximum newly mounted items per frame, including the initial frame; at least one.
    fn mount_budget(&self) -> usize {
        usize::MAX
    }
    /// Lower values mount first (for example squared distance from the viewport center).
    /// Ties follow stacking order; mounting never changes stacking order.
    fn mount_priority(&self, _key: &str) -> u64 {
        0
    }
}

impl El {
    /// Mount only the absolutely positioned children selected by `source`.
    ///
    /// The container takes no authored children and must not also use `virtual_rows` or
    /// `virtualize`. Give it an explicit size: its children do not determine its size.
    /// Window changes alone retain existing subtrees, Live bindings, memos, clocks and focus
    /// without running the root view, including inside Playground when its controls and
    /// Story state are unchanged. New items mount settled, with no entrance replay;
    /// leaving items unmount immediately without exits. A focused item's subtree stays
    /// mounted outside the window until focus leaves it (the source must still build its key).
    /// Hit testing, accessibility and Tab order see mounted children only, including that pin.
    ///
    /// Mounting is spread across frames in priority order, with drawing in `keys` order.
    /// Root commits use the same window and budget but may rebuild its mounted children.
    /// Nested virtual containers, non-indexed virtual lists, changed state/layout reads,
    /// resizing, hot reload and other unsupported retained-layout cases use a full commit.
    /// All of these still build the selected window, including focus pins and the budget.
    pub fn virtual_items(mut self, source: Rc<dyn VirtualItems>) -> Self {
        assert!(self.cold.virtual_estimate.is_none(), "virtual_items cannot also virtualize rows");
        self.cold_mut().virtual_items = Some(source);
        self.flags.set(ElFlags::HAS_VIRTUAL, true);
        self
    }
}
