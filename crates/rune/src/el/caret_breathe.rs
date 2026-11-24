//! Opt-in breathing for a text field's native caret.
use super::El;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Spec {
    pub period: f64,
    pub minimum: f32,
    pub hold: f64,
}

impl El {
    /// Softly pulses only the native caret, keeping text, selection and IME geometry intact.
    /// Editing, moving the insertion point or focusing restarts the fully visible `hold`.
    /// A zero period disables breathing. Reduced motion and composition keep it steady;
    /// an unfocused window hides it. Only a visible caret requests animation frames.
    pub fn caret_breathe(mut self, period: Duration, minimum: f32, hold: Duration) -> Self {
        if let Some(input) = self
            .cold
            .get_mut_if_some()
            .and_then(|c| c.input.as_deref_mut())
        {
            input.caret_breathe = (!period.is_zero()).then_some(Spec {
                period: period.as_secs_f64(),
                minimum: if minimum.is_finite() {
                    minimum.clamp(0.0, 1.0)
                } else {
                    1.0
                },
                hold: hold.as_secs_f64(),
            });
        }
        self
    }
}
