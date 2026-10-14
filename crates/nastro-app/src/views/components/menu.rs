//! The player's menu bar buttons, skinned, opening gpui-kit popup menus in the app chrome style.
//! gpui-kit's `DropdownMenu` works on any element that is `Styled`, `InteractiveElement` and
//! `Selectable`, so [`MenuTrigger`] wraps the skin's flat menu button.

use std::rc::Rc;

use gpui_kit::component::Selectable;
use gpui_kit::component::menu::DropdownMenu;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, Div, ElementId, FontWeight, InteractiveElement, Interactivity, IntoElement, ParentElement,
    RenderOnce, SharedString, Stateful, StatefulInteractiveElement, StyleRefinement, Styled,
    Window, div,
};

use crate::theme::SkinTheme;
use crate::theme::metrics::{
    MENU_BUTTON_HEIGHT, MENU_BUTTON_PADDING, MENU_BUTTON_RADIUS, MENU_SIZE,
};

/// A menu bar button: flat until hovered or open, then the text color at 12 %.
#[derive(IntoElement)]
pub struct MenuTrigger {
    base: Stateful<Div>,
    open: bool,
    theme: Rc<SkinTheme>,
}

impl MenuTrigger {
    /// A trigger labelled `label`.
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        theme: Rc<SkinTheme>,
    ) -> Self {
        let t = &theme;
        let hover = t.colors.text.opacity(0.12);
        let base = div()
            .id(id)
            .h(t.px(MENU_BUTTON_HEIGHT))
            .px(t.px(MENU_BUTTON_PADDING))
            .flex()
            .items_center()
            .rounded(t.px(MENU_BUTTON_RADIUS))
            .text_size(t.px(MENU_SIZE))
            .font_weight(FontWeight::MEDIUM)
            .text_color(t.colors.text)
            .cursor_pointer()
            .hover(move |style| style.bg(hover))
            .child(label.into());
        MenuTrigger {
            base,
            open: false,
            theme,
        }
    }
}

impl MenuTrigger {
    /// A trigger made of any skin control, such as the equalizer's PRESETS button. While its
    /// menu is open it keeps the pressed look.
    pub fn wrap(base: Stateful<Div>, theme: Rc<SkinTheme>) -> Self {
        MenuTrigger {
            base,
            open: false,
            theme,
        }
    }
}

impl Styled for MenuTrigger {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}

impl InteractiveElement for MenuTrigger {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.base.interactivity()
    }
}

impl StatefulInteractiveElement for MenuTrigger {}

impl Selectable for MenuTrigger {
    fn selected(mut self, selected: bool) -> Self {
        self.open = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.open
    }
}

impl DropdownMenu for MenuTrigger {}

impl RenderOnce for MenuTrigger {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let fill = self.theme.colors.text.opacity(0.12);
        self.base.when(self.open, |base| base.bg(fill))
    }
}
