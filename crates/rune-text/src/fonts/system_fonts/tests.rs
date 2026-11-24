use std::path::PathBuf;

use super::*;

fn bundled_font(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rune/assets").join(name)
}

/// A font file already loaded through `load_files` for another purpose still registers its
/// cached `FamilyId`s as CJK fallbacks without reloading the file, while excluding primary
/// families and deduplicating repeated calls.
#[test]
fn register_system_cjk_registers_already_loaded_files() {
    let mut collection = Collection::new(CollectionOptions { shared: false, system_fonts: false });
    let mut loaded = LoadedFiles::new();
    let mono_path = bundled_font("IBMPlexMono-Regular.ttf");
    let primary_path = bundled_font("Inter-Regular.ttf");

    let primary_ids = load_files(&mut collection, &mut loaded, [primary_path.clone()]);
    let mono_ids = load_files(&mut collection, &mut loaded, [mono_path.clone()]);
    assert_eq!(primary_ids.len(), 1);
    assert_eq!(mono_ids.len(), 1);
    let mono_id = mono_ids[0];
    let faces_before = collection.family(mono_id).expect("loaded family").fonts().len();

    let mut all = Vec::new();
    register_system_cjk(
        &mut collection,
        &mut loaded,
        vec![primary_path.clone(), mono_path.clone()],
        ("Inter", "OtherMono"),
        &mut all,
    );

    assert_eq!(all, vec![mono_id], "cached non-primary family should be added to CJK fallbacks");
    assert_eq!(collection.family(mono_id).expect("loaded family").fonts().len(), faces_before, "already-loaded file must not be reloaded");
    for script in [b"Hani", b"Hira", b"Kana", b"Hang", b"Bopo"] {
        let registered: Vec<FamilyId> = collection.fallback_families(FallbackKey::new(Script::from_bytes(*script), None)).collect();
        assert_eq!(registered, vec![mono_id], "script {} should register cached family", std::str::from_utf8(script).unwrap());
    }

    // Calling again with the same already-loaded files keeps `all` deduplicated.
    register_system_cjk(&mut collection, &mut loaded, vec![primary_path, mono_path], ("Inter", "OtherMono"), &mut all);
    assert_eq!(all, vec![mono_id]);
}

/// After `apply_families` switches to missing families on a host with no system fonts, both
/// slots still fall back to the bundled app fonts and measure and draw just like a fresh launch.
#[test]
fn apply_families_missing_family_falls_back_to_bundled_fonts_without_system_fonts() {
    const INTER: &[u8] = include_bytes!("../../../../rune/assets/Inter-Regular.ttf");
    const PLEX: &[u8] = include_bytes!("../../../../rune/assets/IBMPlexMono-Regular.ttf");
    let sans = TextStyle::new(14.0, 400.0);
    let mono = sans.mono();

    let measure_and_draw = |ts: &mut TextSystem, text: &str, style: TextStyle| -> (f32, usize) {
        let layout = ts.layout_rich(text, &[], style, 2.0, None);
        let mut quads = Vec::new();
        ts.glyphs_for_layout(&layout, 2.0, [0.0, 0.0], false, &mut quads);
        (layout.full_width(), quads.len())
    };

    // Strip any host system fonts loaded into `symbol_families` so this test exercises the
    // fontless-host path deterministically on Windows, macOS and Linux alike.
    let mut fresh = TextSystem::new_local(&[INTER, PLEX], ("Rune Missing Family", "Rune Missing Mono"));
    fresh.symbol_families.clear();

    let mut switched = TextSystem::new_local(&[INTER, PLEX], ("Inter", "IBM Plex Mono"));
    let (launch_sans_w, _) = measure_and_draw(&mut switched, "Hamburgefonstiv 0123", sans);
    let (launch_mono_w, _) = measure_and_draw(&mut switched, "let mono = 42;", mono);
    assert!(launch_sans_w > 0.0 && launch_mono_w > 0.0);

    switched.apply_families("Rune Missing Family", "Rune Missing Mono");
    switched.symbol_families.clear();

    for (sample, style) in [("Hamburgefonstiv 0123", sans), ("let mono = 42;", mono)] {
        let (fw, fq) = measure_and_draw(&mut fresh, sample, style);
        let (sw, sq) = measure_and_draw(&mut switched, sample, style);
        assert!(fw > 0.0 && fq > 0, "fresh launch must measure and draw {sample:?}");
        assert_eq!(sw, fw, "after apply_families, {sample:?} must match fresh launch width");
        assert_eq!(sq, fq, "after apply_families, {sample:?} must draw the same glyphs as launch");
    }

    // Switching back restores the original families and widths.
    switched.apply_families("Inter", "IBM Plex Mono");
    assert_eq!(measure_and_draw(&mut switched, "Hamburgefonstiv 0123", sans).0, launch_sans_w);
    assert_eq!(measure_and_draw(&mut switched, "let mono = 42;", mono).0, launch_mono_w);

    // A single missing slot when only one bundled font exists (the default Inter) also falls
    // back to that bundled font on a host with no system fonts, both at launch and after a switch.
    let mut fresh_one = TextSystem::new_local(&[], ("Inter", "Rune Missing Mono"));
    fresh_one.symbol_families.clear();
    let mut switched_one = TextSystem::new_local(&[], ("Inter", "ui-monospace"));
    assert!(switched_one.system_mono.is_none(), "launch must not load system monospace fonts");
    assert!(switched_one.font_files.is_empty(), "launch must not open system font files");
    switched_one.apply_families("Inter", "Rune Missing Mono");
    switched_one.symbol_families.clear();
    let (fw_one, fq_one) = measure_and_draw(&mut fresh_one, "let mono = 42;", mono);
    let (sw_one, sq_one) = measure_and_draw(&mut switched_one, "let mono = 42;", mono);
    assert!(fw_one > 0.0 && fq_one > 0, "fresh single-missing-slot launch must measure and draw");
    assert_eq!(sw_one, fw_one, "switched single-missing-slot must match fresh launch width");
    assert_eq!(sq_one, fq_one, "switched single-missing-slot must draw the same glyphs");
}

#[cfg(target_os = "macos")]
#[test]
fn apple_slot_punctuation_skips_system_fallback_and_emoji_loads_on_demand() {
    if core_text::family_files(&["System Font"]).is_empty() || core_text::family_files(&[APPLE_EMOJI_FAMILY]).is_empty() {
        return;
    }
    // Empty starts without app or embedded fonts, even in a unit test build.
    let mut text = TextSystem::empty();
    text.apply_families("system-ui", "ui-monospace");
    assert!(text.font_cx.collection.family_id(APPLE_EMOJI_FAMILY).is_none());

    let style = TextStyle::new(13.0, 400.0);
    let layout = text.layout_rich("Item · next… threshold ≥ 95%", &[], style, 2.0, None);
    let mut glyph_count = 0;
    for item in layout.lines().flat_map(|line| line.items()) {
        if let PositionedLayoutItem::GlyphRun(run) = item {
            for glyph in run.glyphs() {
                assert_ne!(glyph.id, 0, "punctuation must have a real glyph");
                glyph_count += 1;
            }
        }
    }
    assert!(glyph_count > 0);
    assert!(!text.fallbacks.symbols);
    assert!(text.math.is_none());
    assert!(text.fallbacks.asked.is_empty());
    assert!(text.font_cx.collection.family_id(APPLE_EMOJI_FAMILY).is_none());

    let layout = text.layout_rich("Launch 🚀", &[], style, 2.0, None);
    let mut quads = Vec::new();
    text.glyphs_for_layout(&layout, 2.0, [0.0, 0.0], false, &mut quads);
    assert_eq!(text.emoji, Some(APPLE_EMOJI_FAMILY));
    assert_eq!(text.symbol_families.last(), Some(&APPLE_EMOJI_FAMILY));
    assert_eq!(quads.iter().filter(|quad| quad.color != 0).count(), 1);

    let mut text = TextSystem::empty();
    text.apply_families("system-ui", "ui-monospace");
    text.layout_rich("Launch 🚀\u{FE0E}", &[], style, 2.0, None);
    assert!(text.font_cx.collection.family_id(APPLE_EMOJI_FAMILY).is_none());
}
