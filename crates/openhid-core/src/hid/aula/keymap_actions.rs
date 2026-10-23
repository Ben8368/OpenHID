//! Shared output-action encodings for AULA F87 PRO keymap editing.

use super::{AulaKeymapAction, KEYMAP_FN_LAYER_ACTION_RAW};

/// One named four-byte action accepted by the keymap editors.
///
/// The action bytes describe the editor's current output catalog. Except for
/// the explicitly identified OEM actions, they do not establish that a given
/// firmware accepts the action over a particular transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AulaKeymapActionOutput {
    name: String,
    label: String,
    raw: [u8; 4],
}

impl AulaKeymapActionOutput {
    fn new(name: &str, label: &str, raw: [u8; 4]) -> Self {
        Self {
            name: name.to_owned(),
            label: label.to_owned(),
            raw,
        }
    }

    /// Stable, lowercase action identifier for CLI and profile tooling.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// English fallback label for presentation layers without a translation.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Raw four-byte matrix representation.
    #[must_use]
    pub const fn raw(&self) -> [u8; 4] {
        self.raw
    }
}

/// Return the shared F87 PRO output-action catalog in editor display order.
#[must_use]
pub fn aula_keymap_action_outputs() -> Vec<AulaKeymapActionOutput> {
    let mut outputs = vec![
        output("unbound", "Unbound", [0, 0, 0, 0]),
        output(
            "disable_key",
            "Disable key (OEM)",
            AulaKeymapAction::DisableKey.raw(),
        ),
        output("fn_layer", "Fn layer", KEYMAP_FN_LAYER_ACTION_RAW),
        output("escape", "Escape", keyboard_usage(0x29)),
        output("enter", "Enter", keyboard_usage(0x28)),
        output("space", "Space", keyboard_usage(0x2c)),
        output("tab", "Tab", keyboard_usage(0x2b)),
        output("backspace", "Backspace", keyboard_usage(0x2a)),
        output("caps_lock", "Caps Lock", keyboard_usage(0x39)),
        output("print_screen", "Print Screen", keyboard_usage(0x46)),
        output("scroll_lock", "Scroll Lock", keyboard_usage(0x47)),
        output("pause", "Pause", keyboard_usage(0x48)),
        output("insert", "Insert", keyboard_usage(0x49)),
        output("home", "Home", keyboard_usage(0x4a)),
        output("page_up", "Page Up", keyboard_usage(0x4b)),
        output("delete", "Delete", keyboard_usage(0x4c)),
        output("end", "End", keyboard_usage(0x4d)),
        output("page_down", "Page Down", keyboard_usage(0x4e)),
        output("right", "Right", keyboard_usage(0x4f)),
        output("left", "Left", keyboard_usage(0x50)),
        output("down", "Down", keyboard_usage(0x51)),
        output("up", "Up", keyboard_usage(0x52)),
        output("left_ctrl", "Left Ctrl", modifier(0x01)),
        output("left_shift", "Left Shift", modifier(0x02)),
        output("left_alt", "Left Alt", modifier(0x04)),
        output("left_windows", "Left Windows", modifier(0x08)),
        output("right_ctrl", "Right Ctrl", modifier(0x10)),
        output("right_shift", "Right Shift", modifier(0x20)),
        output("right_alt", "Right Alt", modifier(0x40)),
        output("right_windows", "Right Windows", modifier(0x80)),
        output("media_play_pause", "Media Play/Pause", consumer_usage(0xcd)),
        output("media_next_track", "Media Next Track", consumer_usage(0xb5)),
        output(
            "media_previous_track",
            "Media Previous Track",
            consumer_usage(0xb6),
        ),
        output("media_mute", "Mute", consumer_usage(0xe2)),
        output("media_volume_up", "Volume Up", consumer_usage(0xe9)),
        output("media_volume_down", "Volume Down", consumer_usage(0xea)),
        output("alt_escape", "Alt+Esc", AulaKeymapAction::AltEscape.raw()),
        output("alt_tab", "Alt+Tab", AulaKeymapAction::AltTab.raw()),
    ];

    outputs.extend(character_actions());
    outputs.extend(extended_keyboard_actions());
    outputs.extend(function_actions());
    outputs.extend(keyboard_chord_actions());
    outputs.extend(consumer_actions());

    outputs
}

fn character_actions() -> Vec<AulaKeymapActionOutput> {
    let mut outputs = Vec::new();
    for (usage, character) in (0x04_u8..=0x1d).zip("abcdefghijklmnopqrstuvwxyz".chars()) {
        let name = character.to_string();
        outputs.push(output(&name, &name, keyboard_usage(usage)));
    }
    for (usage, character) in (0x1e_u8..=0x27).zip("1234567890".chars()) {
        let name = character.to_string();
        outputs.push(output(&name, &name, keyboard_usage(usage)));
    }
    outputs
}

fn extended_keyboard_actions() -> Vec<AulaKeymapActionOutput> {
    let mut outputs = Vec::new();
    for (name, label, raw) in [
        ("minus", "Minus", 0x2d),
        ("equals", "Equals", 0x2e),
        ("left_bracket", "Left Bracket", 0x2f),
        ("right_bracket", "Right Bracket", 0x30),
        ("backslash", "Backslash", 0x31),
        ("non_us_hash", "Non-US Hash", 0x32),
        ("semicolon", "Semicolon", 0x33),
        ("apostrophe", "Apostrophe", 0x34),
        ("grave", "Grave", 0x35),
        ("comma", "Comma", 0x36),
        ("period", "Period", 0x37),
        ("slash", "Slash", 0x38),
        ("num_lock", "Num Lock", 0x53),
        ("keypad_slash", "Keypad Slash", 0x54),
        ("keypad_asterisk", "Keypad Asterisk", 0x55),
        ("keypad_minus", "Keypad Minus", 0x56),
        ("keypad_plus", "Keypad Plus", 0x57),
        ("keypad_enter", "Keypad Enter", 0x58),
        ("keypad_1", "Keypad 1", 0x59),
        ("keypad_2", "Keypad 2", 0x5a),
        ("keypad_3", "Keypad 3", 0x5b),
        ("keypad_4", "Keypad 4", 0x5c),
        ("keypad_5", "Keypad 5", 0x5d),
        ("keypad_6", "Keypad 6", 0x5e),
        ("keypad_7", "Keypad 7", 0x5f),
        ("keypad_8", "Keypad 8", 0x60),
        ("keypad_9", "Keypad 9", 0x61),
        ("keypad_0", "Keypad 0", 0x62),
        ("keypad_decimal", "Keypad Decimal", 0x63),
        ("non_us_backslash", "Non-US Backslash", 0x64),
        ("application", "Application", 0x65),
        ("power", "Power", 0x66),
        ("keypad_equals", "Keypad Equals", 0x67),
        ("execute", "Execute", 0x74),
        ("help", "Help", 0x75),
        ("menu", "Menu", 0x76),
        ("select", "Select", 0x77),
        ("stop", "Stop", 0x78),
        ("again", "Again", 0x79),
        ("undo", "Undo", 0x7a),
        ("cut", "Cut", 0x7b),
        ("copy", "Copy", 0x7c),
        ("paste", "Paste", 0x7d),
        ("find", "Find", 0x7e),
        ("keyboard_mute", "Keyboard Mute", 0x7f),
        ("keyboard_volume_up", "Keyboard Volume Up", 0x80),
        ("keyboard_volume_down", "Keyboard Volume Down", 0x81),
        ("locking_caps_lock", "Locking Caps Lock", 0x82),
        ("locking_num_lock", "Locking Num Lock", 0x83),
        ("locking_scroll_lock", "Locking Scroll Lock", 0x84),
        ("keypad_comma", "Keypad Comma", 0x85),
        ("keypad_equal_sign", "Keypad Equal Sign", 0x86),
    ] {
        outputs.push(output(name, label, keyboard_usage(raw)));
    }
    outputs
}

fn function_actions() -> Vec<AulaKeymapActionOutput> {
    let mut outputs = Vec::new();
    for usage in 0x3a_u8..=0x45 {
        let number = usage - 0x39;
        let name = format!("f{number}");
        let label = format!("F{number}");
        outputs.push(output(&name, &label, keyboard_usage(usage)));
    }
    for usage in 0x68_u8..=0x73 {
        let number = usage - 0x5b;
        let name = format!("f{number}");
        let label = format!("F{number}");
        outputs.push(output(&name, &label, keyboard_usage(usage)));
    }
    outputs
}

fn keyboard_chord_actions() -> Vec<AulaKeymapActionOutput> {
    let mut outputs = Vec::new();
    for (usage, character) in (0x04_u8..=0x1d).zip("abcdefghijklmnopqrstuvwxyz".chars()) {
        let name = format!("ctrl_{character}");
        let label = format!("Ctrl+{}", character.to_ascii_uppercase());
        outputs.push(output(&name, &label, modified_keyboard_usage(usage, 0x01)));
    }

    for (name, label, usage, modifiers) in [
        ("ctrl_tab", "Ctrl+Tab", 0x2b, 0x01),
        ("ctrl_shift_tab", "Ctrl+Shift+Tab", 0x2b, 0x03),
        ("ctrl_shift_escape", "Ctrl+Shift+Esc", 0x29, 0x03),
        ("alt_f4", "Alt+F4", 0x3d, 0x04),
        ("ctrl_f4", "Ctrl+F4", 0x3d, 0x01),
        ("shift_delete", "Shift+Delete", 0x4c, 0x02),
        ("ctrl_insert", "Ctrl+Insert", 0x49, 0x01),
        ("shift_insert", "Shift+Insert", 0x49, 0x02),
        ("windows_d", "Windows+D", 0x07, 0x08),
        ("windows_e", "Windows+E", 0x08, 0x08),
        ("windows_l", "Windows+L", 0x0f, 0x08),
        ("windows_r", "Windows+R", 0x15, 0x08),
    ] {
        outputs.push(output(
            name,
            label,
            modified_keyboard_usage(usage, modifiers),
        ));
    }
    outputs
}

fn consumer_actions() -> Vec<AulaKeymapActionOutput> {
    let mut outputs = Vec::new();
    for (name, label, raw) in [
        ("launch_email", "Email", 0x018a),
        ("launch_calculator", "Calculator", 0x0192),
        ("launch_media_player", "Media Player", 0x0193),
        ("launch_computer", "My Computer", 0x0194),
        ("launch_home_page", "Home Page", 0x0196),
        ("launch_favorites", "Favorites", 0x022a),
        ("display_brightness_up", "Display Brightness Up", 0x006f),
        ("display_brightness_down", "Display Brightness Down", 0x0070),
        ("media_play", "Media Play", 0xb0),
        ("media_pause", "Media Pause", 0xb1),
        ("media_record", "Media Record", 0xb2),
        ("media_fast_forward", "Media Fast Forward", 0xb3),
        ("media_rewind", "Media Rewind", 0xb4),
        ("media_stop", "Media Stop", 0xb7),
        ("media_eject", "Media Eject", 0xb8),
        ("media_random_play", "Media Random Play", 0xb9),
        ("media_repeat", "Media Repeat", 0xbc),
        ("media_stop_eject", "Media Stop/Eject", 0xcc),
        ("media_play_skip", "Media Play/Skip", 0xce),
        ("media_voice_command", "Media Voice Command", 0xcf),
        ("bass_boost", "Bass Boost", 0xe5),
        ("surround_mode", "Surround Mode", 0xe6),
        ("loudness", "Loudness", 0xe7),
        ("mpx", "MPX", 0xe8),
    ] {
        outputs.push(output(name, label, consumer_usage(raw)));
    }
    outputs
}

/// Resolve a named action or modifier/key chord to its raw output bytes.
///
/// Chords use `+`-separated modifiers followed by a catalog keyboard action,
/// for example `ctrl+shift+a`, `alt+f4`, or `windows+r`. Modifier aliases are
/// `control` for `ctrl` and `win` for `windows`; names are case-insensitive.
/// This describes the editor's encoding and does not prove firmware support.
#[must_use]
pub fn aula_keymap_action_output(name: &str) -> Option<[u8; 4]> {
    let normalized = name.trim().to_ascii_lowercase();
    let outputs = aula_keymap_action_outputs();
    if let Some(output) = outputs.iter().find(|output| output.name == normalized) {
        return Some(output.raw);
    }

    let mut modifiers = 0;
    let mut found_modifier = false;
    let mut key = None;

    for part in normalized.split('+').map(str::trim) {
        let mask = match part {
            "ctrl" | "control" => Some(0x01),
            "shift" => Some(0x02),
            "alt" => Some(0x04),
            "win" | "windows" => Some(0x08),
            _ => None,
        };
        if let Some(mask) = mask {
            if key.is_some() || modifiers & mask != 0 {
                return None;
            }
            modifiers |= mask;
            found_modifier = true;
            continue;
        }

        if key.is_some() {
            return None;
        }
        key = Some(part);
    }

    if !found_modifier {
        return None;
    }
    let key = key?;
    let usage =
        outputs
            .iter()
            .find(|output| output.name == key)
            .and_then(|output| match output.raw {
                [0, 0, 0, usage] if usage != 0 => Some(usage),
                _ => None,
            })?;
    Some(modified_keyboard_usage(usage, modifiers))
}

/// Return a readable label for a catalog action or standard keyboard chord.
///
/// Unknown four-byte actions remain unlabeled so callers can preserve and
/// display their raw representation without implying that it is understood.
#[must_use]
pub fn aula_keymap_action_label(raw: [u8; 4]) -> Option<String> {
    let outputs = aula_keymap_action_outputs();
    if let Some(output) = outputs.iter().find(|output| output.raw == raw) {
        return Some(output.label.clone());
    }

    let [usage, 0, modifiers, 0] = raw else {
        return None;
    };
    if modifiers == 0 || modifiers & !0x0f != 0 {
        return None;
    }
    let key = outputs
        .iter()
        .find(|output| matches!(output.raw, [0, 0, 0, action_usage] if action_usage == usage))?;

    let mut labels = Vec::with_capacity(5);
    for (mask, label) in [
        (0x01, "Ctrl"),
        (0x02, "Shift"),
        (0x04, "Alt"),
        (0x08, "Windows"),
    ] {
        if modifiers & mask != 0 {
            labels.push(label.to_owned());
        }
    }
    labels.push(
        if key.name.len() == 1 && key.name.as_bytes()[0].is_ascii_alphabetic() {
            key.name.to_ascii_uppercase()
        } else {
            key.label.clone()
        },
    );
    Some(labels.join("+"))
}

fn output(name: &str, label: &str, raw: [u8; 4]) -> AulaKeymapActionOutput {
    AulaKeymapActionOutput::new(name, label, raw)
}

const fn keyboard_usage(usage: u8) -> [u8; 4] {
    [0, 0, 0, usage]
}

const fn consumer_usage(usage: u16) -> [u8; 4] {
    let [low, high] = usage.to_le_bytes();
    [0x02, 0, high, low]
}

const fn modified_keyboard_usage(usage: u8, modifiers: u8) -> [u8; 4] {
    [usage, 0, modifiers, 0]
}

const fn modifier(mask: u8) -> [u8; 4] {
    [0, mask, 0, 0]
}
