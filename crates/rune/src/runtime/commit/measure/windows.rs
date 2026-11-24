use super::{El, ElFlags, node_path};

#[derive(Clone, Copy)]
pub(super) struct ScrollCtx<'a> {
    pub(super) path: &'a str,
    pub(super) sticky: bool,
    pub(super) bound: f32,
    pub(super) nested: bool,
    pub(super) no_anchor: bool,
}

pub(super) struct OwnedScrollCtx {
    path: String,
    sticky: bool,
    bound: f32,
    nested: bool,
    no_anchor: bool,
}

impl OwnedScrollCtx {
    fn from_ctx(c: ScrollCtx<'_>) -> Self {
        Self { path: c.path.to_owned(), sticky: c.sticky, bound: c.bound, nested: c.nested, no_anchor: c.no_anchor }
    }

    pub(super) fn as_ref(&self) -> ScrollCtx<'_> {
        ScrollCtx { path: self.path.as_str(), sticky: self.sticky, bound: self.bound, nested: self.nested, no_anchor: self.no_anchor }
    }
}

/// The `virtual_rows` lists in memo placeholder `el`'s shared subtree at `path`, with the scroll
/// viewport each windows against, as `window_tree` would find them. False when one can't be
/// windowed without expanding: a child-built list, an item window, or a memo with extra children.
pub(super) fn windowed_lists<'e>(el: &'e El, path: &str, scroll: Option<ScrollCtx<'_>>, bound: f32, out: &mut Vec<(String, &'e El, Option<OwnedScrollCtx>)>) -> bool {
    if !el.flags.has(ElFlags::HAS_VIRTUAL) {
        return true;
    }
    // An item window is selected while its container expands.
    if el.cold.virtual_items.is_some() {
        return false;
    }
    let kids: &[El] = match el.memo_children() {
        Some(m) if el.children.is_empty() && !el.flags.has(ElFlags::MEMO_EXTENDED) => &m.children,
        Some(_) => return false,
        None => &el.children,
    };
    let bound = crate::el::virtual_list::height_bound(&el.style, bound);
    let is_virtual = el.cold.virtual_estimate.is_some();
    if is_virtual {
        if el.cold.virtual_rows.is_none() {
            return false;
        }
        out.push((path.to_owned(), el, scroll.map(OwnedScrollCtx::from_ctx)));
    }
    let bounds = crate::el::virtual_list::child_bounds(&el.style, el.cold.scroll.is_some(), bound, kids);
    let scroll = match el.cold.scroll {
        Some(s) => Some(ScrollCtx { path, sticky: s.stick_bottom, bound, nested: false, no_anchor: el.cold.scroll_edges.no_anchor }),
        None => scroll.map(|sc| ScrollCtx { nested: sc.nested || is_virtual, ..sc }),
    };
    kids.iter().enumerate().filter(|(_, c)| c.flags.has(ElFlags::HAS_VIRTUAL)).all(|(i, child)| {
        windowed_lists(child, &node_path(path, child.key.as_deref(), i), scroll, bounds.of(child), out)
    })
}
