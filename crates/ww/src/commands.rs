//! The command bar of a tab as data: the window draws the items, the program names them.

use crate::keys::Keys;

/// An item of the command bar.
pub enum Item<T> {
    /// A button; without an act it is disabled.
    Command(Command<T>),
    Separator,
    /// A button that opens a menu the program draws (`App::menu_popup`), by its number.
    Menu(MenuButton),
}

/// A button of the command bar.
pub struct Command<T> {
    pub icon: &'static str,
    /// The label, translated.
    pub label: &'static str,
    pub keys: Option<&'static Keys>,
    /// What a click does; `None` disables the button.
    pub act: Option<T>,
    /// A switch that is on: the button is drawn pressed.
    pub pressed: bool,
}

/// A button of the command bar that opens a menu.
pub struct MenuButton {
    pub icon: &'static str,
    /// The label, translated.
    pub label: &'static str,
    /// The least width of the menu.
    pub width: f32,
    /// The number the program knows the menu by.
    pub id: usize,
}

/// A command with an act.
pub fn cmd<T>(icon: &'static str, label: &'static str, act: impl Into<T>) -> Item<T> {
    Item::Command(Command {
        icon,
        label,
        keys: None,
        act: Some(act.into()),
        pressed: false,
    })
}

/// A command that is disabled without an act.
pub fn cmd_if<T>(icon: &'static str, label: &'static str, act: Option<T>) -> Item<T> {
    Item::Command(Command {
        icon,
        label,
        keys: None,
        act,
        pressed: false,
    })
}

/// A switch of the command bar: a click turns it on or off, and it is drawn pressed when on.
pub fn toggle<T>(
    icon: &'static str,
    label: &'static str,
    pressed: bool,
    act: impl Into<T>,
) -> Item<T> {
    Item::Command(Command {
        icon,
        label,
        keys: None,
        act: Some(act.into()),
        pressed,
    })
}

/// A command enabled on a condition.
pub fn when<T>(
    icon: &'static str,
    label: &'static str,
    enabled: bool,
    act: impl Into<T>,
) -> Item<T> {
    cmd_if(icon, label, enabled.then(|| act.into()))
}

/// A command with its keys.
pub fn keyed<T>(
    icon: &'static str,
    label: &'static str,
    keys: &'static Keys,
    act: impl Into<T>,
) -> Item<T> {
    Item::Command(Command {
        icon,
        label,
        keys: Some(keys),
        act: Some(act.into()),
        pressed: false,
    })
}

/// A button that opens a menu.
pub fn menu<T>(icon: &'static str, label: &'static str, width: f32, id: usize) -> Item<T> {
    Item::Menu(MenuButton {
        icon,
        label,
        width,
        id,
    })
}
