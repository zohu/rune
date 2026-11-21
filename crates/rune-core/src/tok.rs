//! Design tokens: the one place for colour, space, radius, type and motion.

use crate::color::{Color, oklch, oklcha};

pub mod motion {
    pub use rune_motion::motion::*;
    /// The leading edge of something travelling (width leads).
    pub const LEAD: rune_motion::Spring = rune_motion::Spring::new(0.2, 0.0);
}

pub mod space {
    pub const S1: f32 = 4.0;
    pub const S2: f32 = 8.0;
    pub const S3: f32 = 12.0;
    pub const S6: f32 = 24.0;
}

pub mod radius {
    pub const SM: f32 = 6.0;
    pub const MD: f32 = 8.0;
    pub const LG: f32 = 12.0;
}

/// An app font slot (`AppSpec::families`) or a family registered through `AppSpec::fonts`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Family {
    #[default]
    Sans,
    Mono,
    /// A font family by name, registered or installed (an installed one loads on first use),
    /// falling back to the sans slot if missing. With a comma it is a CSS family list, its
    /// families tried in order before the sans slot: `"Iosevka", JetBrains Mono`.
    Named(&'static str),
    /// The platform's monospaced UI font, loaded from the system on first use, never bundled:
    /// SF Mono on macOS (AppKit's `monospacedSystemFont`, Menlo before it existed), Cascadia
    /// Mono then Consolas on Windows, and fontconfig's `monospace` on Linux. Falls back to the
    /// mono slot when none is installed (headless on a bare machine, the web).
    SystemMono,
}

/// Maximum number of distinct runtime font family names interned by [`Family::named_bounded`].
const MAX_INTERNED_FAMILIES: usize = 512;

fn intern_bounded(
    names: &mut std::collections::HashSet<&'static str>,
    cap: usize,
    name: &str,
) -> Option<Family> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(&known) = names.get(trimmed) {
        return Some(Family::Named(known));
    }
    if names.len() < cap {
        let leaked: &'static str = Box::leak(trimmed.into());
        names.insert(leaked);
        return Some(Family::Named(leaked));
    }
    None
}

impl Family {
    /// Interns a runtime font family name into a process-wide deduplicated table capped at 512
    /// distinct names, preserving [`Family`]'s `Copy` representation.
    ///
    /// Leading and trailing whitespace is trimmed. Returns `None` for empty or whitespace-only
    /// names, or once the table has reached capacity for a name not already interned (already
    /// interned names continue to reuse their pointer even at capacity).
    pub fn named_bounded(name: &str) -> Option<Family> {
        static NAMES: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<&'static str>>> =
            std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));
        let mut names = NAMES.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        intern_bounded(&mut names, MAX_INTERNED_FAMILIES, name)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum OpticalSize {
    /// Use the run's point size, independent of display scale. Explicit `opsz` variations win.
    #[default]
    Auto,
    /// Leave optical sizing to Parley/the font (the legacy Rune behavior).
    FontDefault,
    /// Override `opsz`, in points, including an `opsz` in the variation settings. Also picks the
    /// `trak` tracking: text is tracked as Core Text tracks text of this size scaled to the set
    /// size.
    Fixed(f32),
}

/// Hashes as `==` compares, `-0.0` as `0.0`, so a style can be a `memo` version.
impl std::hash::Hash for OpticalSize {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        if let OpticalSize::Fixed(size) = self { crate::color::hash_bits(*size).hash(state) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    pub size: f32,
    pub weight: f32,
    pub family: Family,
    /// Line height in points; 0 means 1.25 × size.
    pub line_height: f32,
    /// Letter spacing in points, added to the font's own size-specific tracking (see `features`).
    pub tracking: f32,
    /// CSS OpenType settings, e.g. `"'opsz' 14, 'wght' 500"`. Empty preserves font defaults.
    pub variations: &'static str,
    /// CSS OpenType settings, e.g. `"'tnum' 1, 'ss01' 1"`. Fonts with a `trak` table (the macOS
    /// system font among them) are tracked at their point size (or explicit optical size), as Core
    /// Text does; `'trak' 0` turns that off.
    pub features: &'static str,
    pub optical_size: OpticalSize,
    /// The family's italic face, or a synthetic oblique when it has none.
    pub italic: bool,
}

/// Hashes as `==` compares, `-0.0` as `0.0`, so a style can be a `memo` version.
impl std::hash::Hash for TextStyle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let TextStyle { size, weight, family, line_height, tracking, variations, features, optical_size, italic } = *self;
        [size, weight, line_height, tracking].map(crate::color::hash_bits).hash(state);
        (family, variations, features, optical_size, italic).hash(state);
    }
}

impl TextStyle {
    pub const fn new(size: f32, weight: f32) -> TextStyle {
        TextStyle { size, weight, family: Family::Sans, line_height: 0.0, tracking: 0.0,
            variations: "", features: "", optical_size: OpticalSize::Auto, italic: false }
    }
    pub const fn italic(self) -> TextStyle {
        TextStyle { italic: true, ..self }
    }
    pub const fn mono(self) -> TextStyle {
        TextStyle { family: Family::Mono, ..self }
    }
    pub const fn family(self, family: Family) -> TextStyle {
        TextStyle { family, ..self }
    }
    pub const fn lh(self, line_height: f32) -> TextStyle {
        TextStyle { line_height, ..self }
    }
    pub const fn tracking(self, tracking: f32) -> TextStyle {
        TextStyle { tracking, ..self }
    }
    pub const fn variations(self, variations: &'static str) -> TextStyle {
        TextStyle { variations, ..self }
    }
    pub const fn features(self, features: &'static str) -> TextStyle {
        TextStyle { features, ..self }
    }
    /// Fixes `opsz` at `size` points and picks the `trak` tracking with it: text is tracked as
    /// Core Text tracks `size`-point text scaled to the set size, so
    /// `TextStyle::new(size * k, w).optical_size(size)` matches a SwiftUI view at `size` with
    /// `scaleEffect(k)`. Auto optical size tracks at the point size. To change only the outlines
    /// and keep the set size's tracking, as CoreText does for a font given an `opsz` variation,
    /// use `.variations("'opsz' 17")`: on SF at 16 pt, `.optical_size(17.0)` is 0.094 pt a glyph
    /// tighter (17-pt tracking scaled to 16), and both match CoreText's widths exactly.
    pub const fn optical_size(self, size: f32) -> TextStyle {
        TextStyle { optical_size: OpticalSize::Fixed(size), ..self }
    }
    pub const fn optical_sizing(self, optical_size: OpticalSize) -> TextStyle {
        TextStyle { optical_size, ..self }
    }
    /// Resolved line height in points.
    pub fn line_px(&self) -> f32 {
        if self.line_height > 0.0 { self.line_height } else { self.size * 1.25 }
    }
}

pub mod type_ {
    use super::TextStyle;
    pub const BODY: TextStyle = TextStyle::new(13.0, 400.0);
    pub const LABEL: TextStyle = TextStyle::new(12.0, 500.0);
    pub const SECTION: TextStyle = TextStyle::new(11.5, 600.0);
    pub const TITLE: TextStyle = TextStyle::new(13.0, 600.0);
    pub const MONO_CAPTION: TextStyle = TextStyle::new(12.0, 500.0);
}

/// Semantic colours for one appearance. Alphas are for linear-light blending, so white overlays
/// on dark are much lower and black overlays on light much higher than their sRGB/CSS equivalents.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub bg: Color,
    pub card: Color,
    pub card_border: Color,
    pub text: Color,
    pub text_secondary: Color,
    pub separator: Color,
    pub control_track: Color,
    pub control_off: Color,
    pub indicator: Color,
    pub knob: Color,
    pub hover: Color,
    pub press: Color,
    pub shadow: Color,
    pub shadow_soft: Color,
    /// Hairline contact shadow under raised controls.
    pub edge: Color,
    pub knob_shadow: Color,
}

pub fn theme(dark: bool) -> Theme {
    if dark {
        Theme {
            bg: oklch(0.185, 0.004, 265.0),
            card: oklch(0.232, 0.005, 265.0),
            card_border: oklcha(1.0, 0.0, 0.0, 0.05),
            text: oklch(0.955, 0.003, 265.0),
            text_secondary: oklch(0.72, 0.008, 265.0),
            separator: oklcha(1.0, 0.0, 0.0, 0.055),
            control_track: oklcha(1.0, 0.0, 0.0, 0.05),
            control_off: oklcha(1.0, 0.0, 0.0, 0.085),
            indicator: oklch(0.43, 0.006, 265.0),
            knob: oklch(0.985, 0.0, 0.0),
            hover: oklcha(1.0, 0.0, 0.0, 0.016),
            press: oklcha(1.0, 0.0, 0.0, 0.03),
            shadow: oklcha(0.0, 0.0, 0.0, 0.42),
            shadow_soft: oklcha(0.0, 0.0, 0.0, 0.28),
            edge: oklcha(0.0, 0.0, 0.0, 0.3),
            knob_shadow: oklcha(0.0, 0.0, 0.0, 0.3),
        }
    } else {
        Theme {
            bg: oklch(0.958, 0.004, 265.0),
            card: oklch(1.0, 0.0, 0.0),
            card_border: oklcha(0.0, 0.0, 0.0, 0.1),
            text: oklch(0.225, 0.006, 265.0),
            text_secondary: oklch(0.5, 0.01, 265.0),
            separator: oklcha(0.0, 0.0, 0.0, 0.13),
            control_track: oklcha(0.2, 0.01, 265.0, 0.13),
            control_off: oklcha(0.2, 0.01, 265.0, 0.24),
            indicator: oklch(1.0, 0.0, 0.0),
            knob: oklch(1.0, 0.0, 0.0),
            hover: oklcha(0.2, 0.01, 265.0, 0.07),
            press: oklcha(0.2, 0.01, 265.0, 0.12),
            shadow: oklcha(0.2, 0.02, 265.0, 0.16),
            shadow_soft: oklcha(0.2, 0.02, 265.0, 0.07),
            edge: oklcha(0.2, 0.02, 265.0, 0.2),
            knob_shadow: oklcha(0.2, 0.02, 265.0, 0.3),
        }
    }
}

pub const ACCENT_NAMES: [&str; 6] = ["blue", "purple", "pink", "orange", "green", "graphite"];

pub fn accent(i: usize, dark: bool) -> Color {
    let l = if dark { 0.66 } else { 0.6 };
    match i % 6 {
        0 => oklch(l, 0.17, 256.0),
        1 => oklch(l, 0.18, 300.0),
        2 => oklch(l + 0.02, 0.19, 357.0),
        3 => oklch(l + 0.08, 0.16, 58.0),
        4 => oklch(l + 0.04, 0.16, 152.0),
        _ => oklch(l - 0.02, 0.012, 265.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_bounded_trims_deduplicates_reuses_at_cap_and_caps() {
        assert_eq!(Family::named_bounded(""), None);
        assert_eq!(Family::named_bounded("   \t\n "), None);

        let a = Family::named_bounded("  JetBrains Mono  ");
        let b = Family::named_bounded("JetBrains Mono");
        match (a, b) {
            (Some(Family::Named(pa)), Some(Family::Named(pb))) => {
                assert_eq!(pa, "JetBrains Mono");
                assert!(std::ptr::eq(pa, pb));
            }
            other => panic!("expected reused interned pointer, got {other:?}"),
        }

        let mut table = std::collections::HashSet::new();
        assert_eq!(intern_bounded(&mut table, MAX_INTERNED_FAMILIES, ""), None);
        assert_eq!(intern_bounded(&mut table, MAX_INTERNED_FAMILIES, " \t "), None);

        let first = intern_bounded(&mut table, MAX_INTERNED_FAMILIES, "  Font-0  ");
        let first_dup = intern_bounded(&mut table, MAX_INTERNED_FAMILIES, "Font-0");
        match (first, first_dup) {
            (Some(Family::Named(pa)), Some(Family::Named(pb))) => {
                assert_eq!(pa, "Font-0");
                assert!(std::ptr::eq(pa, pb));
            }
            other => panic!("expected deduplicated pointer, got {other:?}"),
        }

        for i in 1..MAX_INTERNED_FAMILIES {
            let name = format!("Font-{i}");
            assert!(matches!(
                intern_bounded(&mut table, MAX_INTERNED_FAMILIES, &name),
                Some(Family::Named(stored)) if stored == name
            ));
        }
        assert_eq!(table.len(), MAX_INTERNED_FAMILIES);

        // Existing entries still reuse their interned pointer at capacity.
        match (first, intern_bounded(&mut table, MAX_INTERNED_FAMILIES, "Font-0")) {
            (Some(Family::Named(pa)), Some(Family::Named(pb))) => assert!(std::ptr::eq(pa, pb)),
            other => panic!("expected pointer reuse at capacity, got {other:?}"),
        }

        // New distinct names past capacity return None without growing the table.
        assert_eq!(intern_bounded(&mut table, MAX_INTERNED_FAMILIES, "Font-Overflow"), None);
        assert_eq!(table.len(), MAX_INTERNED_FAMILIES);
    }
}
