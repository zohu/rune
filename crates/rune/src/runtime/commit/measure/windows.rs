use super::*;

/// The `virtual_rows` lists in memo placeholder `el`'s shared subtree at `path`, with the scroll
/// viewport each windows against, as `window_tree` would find them. False when one can't be
/// windowed without expanding: a child-built list, an item window, or a memo with extra children.
pub(super) fn windowed_lists<'e>(el: &'e El, path: &str, scroll: Option<(&str, bool, f32, bool)>, bound: f32, out: &mut Vec<(String, &'e El, Option<(String, bool, f32, bool)>)>) -> bool {
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
        out.push((path.to_owned(), el, scroll.map(|(p, s, b, n)| (p.to_owned(), s, b, n))));
    }
    let bounds = crate::el::virtual_list::child_bounds(&el.style, el.cold.scroll.is_some(), bound, kids);
    let scroll = match el.cold.scroll {
        Some(s) => Some((path, s.stick_bottom, bound, false)),
        None => scroll.map(|(p, s, b, n)| (p, s, b, n || is_virtual)),
    };
    kids.iter().enumerate().filter(|(_, c)| c.flags.has(ElFlags::HAS_VIRTUAL)).all(|(i, child)| {
        windowed_lists(child, &node_path(path, child.key.as_deref(), i), scroll, bounds.of(child), out)
    })
}
