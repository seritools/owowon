use egui::{Key, KeyboardShortcut, Modifiers};

pub const TOGGLE_MEASUREMENT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::M);

pub const ZOOM_IN: Key = Key::PageUp;
pub const ZOOM_OUT: Key = Key::PageDown;

pub const HORIZONTAL_OFFSET_LEFT: Key = Key::ArrowLeft;
pub const HORIZONTAL_OFFSET_RIGHT: Key = Key::ArrowRight;

pub const VERTICAL_OFFSET_UP: Key = Key::ArrowUp;
pub const VERTICAL_OFFSET_DOWN: Key = Key::ArrowDown;
