//! Item windows join the same disjoint subtree move plan as indexed rows.
use super::*;

impl Measurement<'_> {
    pub(super) fn plan_items(&mut self, plans: &mut Vec<ListPlan>) {
        for (path, v) in &mut self.items.lists {
            let old_list_idx = self.node_index[&self.path_ids[path]];
            let mut end = old_list_idx + 1;
            while end < self.nodes.len() && self.nodes[end].parent.is_some_and(|p| p >= old_list_idx) {
                end += 1;
            }
            let mut mounted_rows = HashMap::default();
            let mut cur = old_list_idx + 1;
            while cur < end {
                let mut row_end = cur + 1;
                while row_end < end && self.nodes[row_end].parent.is_some_and(|p| p >= cur) {
                    row_end += 1;
                }
                if let Some(key) = &self.nodes[cur].key {
                    mounted_rows.insert(key.clone(), (cur, row_end));
                }
                cur = row_end;
            }
            let keys = v.select(path, self.focused, self.items.frame);
            for key in v.shown.iter().filter(|k| !keys.contains(k)) {
                self.virtual_culled.insert(format!("{path}/{key}"));
            }
            plans.push(ListPlan {
                range: 0..keys.len(),
                keys,
                path: path.clone(),
                old_list_idx,
                list_subtree_end: end,
                pad_top: 0.0,
                pad_bottom: 0.0,
                mounted_rows,
                focused: None,
                pinned: Vec::new(),
                source: Source::Items(v.source.clone()),
                list_group: None,
                inert: false,
                quiet: Vec::new(),
                built_rows: Vec::new(),
            });
        }
    }
}

pub(super) fn refresh_live_text(n: &mut Node, scale: f32) {
    let Some(spec) = &mut n.text else { return };
    let Some(live) = &spec.cold.live else { return };
    // Retained nodes skip prepare_text. Keep the authored measurement separate from live
    // glyphs: a remount must neither replay stale glyphs nor resize the placeholder's box.
    n.cold.live_text_measure.get_or_insert_with(|| spec.clone());
    if let Some(value) = live.read(&mut n.cold.live_text_seen)
        && spec.source_text() != &*value
    {
        spec.replace_content(value.to_string());
        spec.apply_case();
    }
    if spec.shape.is_none() {
        spec.key_shape(scale);
    }
}
