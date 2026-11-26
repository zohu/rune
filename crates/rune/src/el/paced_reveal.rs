//! Reading-order reveal for one streaming document.

use std::{fmt, rc::Rc};

use crate::{El, Live};

#[derive(Clone)]
pub(crate) struct Pace {
    pub fade: f32,
    pub interval: f32,
    pub max_delay: f32,
    pub caret: Option<Live<[f32; 2]>>,
}

impl fmt::Debug for Pace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pace").field("fade", &self.fade).field("interval", &self.interval)
            .field("max_delay", &self.max_delay).finish_non_exhaustive()
    }
}

impl El {
    /// Pace newly appended display text marked with [`El::reveal`] in this subtree, in reading
    /// order. Each Unicode word (a grapheme for Han and emoji) or source line of `by_line` code
    /// fades for `fade` seconds. Starts are `interval` seconds apart, compressed to fit within
    /// `max_delay` seconds of arrival. Zero interval fades an append together; zero fade lands it.
    /// The optional live point follows the readable edge in this box's logical coordinates,
    /// at the vertical middle of its shaped line. Give each simultaneous message its own point.
    pub fn paced_reveal(mut self, fade: f32, interval: f32, max_delay: f32, caret: Option<&Live<[f32; 2]>>) -> Self {
        let seconds = |v: f32| if v.is_finite() { v.max(0.0) } else { 0.0 };
        self.vis.cold_mut().paced_reveal = Some(Rc::new(Pace {
            fade: seconds(fade), interval: seconds(interval), max_delay: seconds(max_delay), caret: caret.cloned(),
        }));
        self
    }
}
