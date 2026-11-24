//! The lightweight Apple slot faces and the separate on-demand colour emoji face.

use parley::fontique::{Blob, CharmapIndex};

use crate::TextSystem;

use super::{APPLE_EMOJI_FAMILY, core_text, load_files, present_families};

pub(super) type SlotCharmap = (Blob<u8>, CharmapIndex);

const SLOT_FAMILIES: &[&str] = &["System Font", ".SF NS", "Menlo"];

impl TextSystem {
    pub(super) fn load_apple_slot_fallbacks(&mut self) {
        if self.fallbacks.apple_slots.is_some() {
            return;
        }
        load_files(
            &mut self.font_cx.collection,
            &mut self.font_files,
            core_text::family_files(SLOT_FAMILIES),
        );
        let mut maps = Vec::new();
        for name in present_families(&mut self.font_cx.collection, SLOT_FAMILIES) {
            self.push_symbol_family(name, false);
            let collection = &mut self.font_cx.collection;
            let Some(family) = collection
                .family_id(name)
                .and_then(|id| collection.family(id))
            else {
                continue;
            };
            let Some(font) = family.default_font() else {
                continue;
            };
            if let Some(data) = font.load(Some(&mut self.font_cx.source_cache)) {
                maps.push((data, font.charmap_index()));
            }
        }
        self.fallbacks.apple_slots = Some(maps);
    }

    pub(super) fn load_apple_emoji(&mut self) {
        load_files(
            &mut self.font_cx.collection,
            &mut self.font_files,
            core_text::family_files(&[APPLE_EMOJI_FAMILY]),
        );
        if self
            .font_cx
            .collection
            .family_id(APPLE_EMOJI_FAMILY)
            .is_some()
        {
            self.push_symbol_family(APPLE_EMOJI_FAMILY, true);
        }
    }
}
