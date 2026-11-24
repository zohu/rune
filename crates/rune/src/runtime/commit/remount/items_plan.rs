//! Item windows join the same disjoint subtree move plan as indexed rows.
use super::*;

impl Measurement<'_> {
    pub(super) fn plan_items(&mut self, plans: &mut Vec<ListPlan>) {
        for (path, window) in &mut self.items.lists {
            let old_list_idx = self.node_index[&self.path_ids[path]];
            let (list_subtree_end, mounted_rows) = mounted_children(self.nodes, old_list_idx);
            let keys = window.select(path, self.focused, self.items.frame);
            window.cull_departed(path, &keys, self.virtual_culled);
            plans.push(ListPlan {
                range: 0..keys.len(),
                path: path.clone(),
                old_list_idx,
                list_subtree_end,
                pad_top: 0.0,
                pad_bottom: 0.0,
                mounted_rows,
                focused: None,
                pinned: Vec::new(),
                source: Source::Items { source: window.source.clone(), keys },
                list_group: None,
                inert: false,
                quiet: Vec::new(),
                built_rows: Vec::new(),
            });
        }
    }
}
