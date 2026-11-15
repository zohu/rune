//! Text field configuration.
use super::*;

/// A text field bound to a [`TextEdit`] in app state.
#[derive(Clone)]
pub struct InputSpec {
    /// Text edits are immediate, but dependent layout (e.g. filtered results) may animate.
    pub animate_layout: bool,
    pub edit: TextEdit,
    pub placeholder: String,
    pub placeholder_color: Color,
    pub caret: Color,
    pub(crate) caret_breathe: Option<caret_breathe::Spec>,
    pub selection: Color,
    /// One unwrapped line that scrolls sideways to keep the caret visible, like `<input>`.
    pub single_line: bool,
    /// `placeholder_color`, and `caret` with `selection`, came from [`El::placeholder`] and
    /// [`El::caret`]; the others follow the window's foreground ([`Cx::foreground`](crate::Cx::foreground)).
    pub(crate) placeholder_inked: bool,
    pub(crate) caret_inked: bool,
    pub(crate) revealed_override: Option<bool>,
    pub(crate) decorations: Vec<Span>,
    pub(crate) rich_style: crate::edit::RichStyle,
    pub(crate) pill_icon: Option<PillIconFn>,
    pub(crate) pill_style: Option<PillStyleFn>,
    pub(crate) access: Option<EditFn>,
    pub(crate) submit: Option<PressFn>,
    pub(crate) key: Option<InputKeyFn>,
    pub(crate) change: Option<PressFn>,
    pub(crate) paste: Option<InputPasteFn>,
    /// `El::spellcheck`: the platform's checker marks misspelled words.
    pub(crate) spellcheck: bool,
    /// [`El::selection_style`]: the selection's highlight and ink, by window focus, over `selection`.
    pub(crate) selection_style: Option<SelectionStyle>,
}

impl std::fmt::Debug for InputSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputSpec")
            .field("animate_layout", &self.animate_layout)
            .field("edit", &self.edit)
            .field("placeholder", &self.placeholder)
            .field("placeholder_color", &self.placeholder_color)
            .field("caret", &self.caret)
            .field("caret_breathe", &self.caret_breathe)
            .field("selection", &self.selection)
            .field("single_line", &self.single_line)
            .finish_non_exhaustive()
    }
}
