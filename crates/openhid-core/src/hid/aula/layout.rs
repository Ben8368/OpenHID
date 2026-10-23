//! OEM-defined F87 PRO key legends and RGB LED positions.

/// Position of one physical key in the F87 PRO OEM keyboard layout.
///
/// Coordinates use the installed `F87PRO/KB.ini` 600×231 design space. The
/// `led_index` is used by the custom RGB table and matches the captured base
/// matrix slot for each of the 87 physical keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AulaKeyPosition {
    label: &'static str,
    led_index: u8,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
}

impl AulaKeyPosition {
    const fn new(
        label: &'static str,
        led_index: u8,
        x: u16,
        y: u16,
        width: u16,
        height: u16,
    ) -> Self {
        Self {
            label,
            led_index,
            x,
            y,
            width,
            height,
        }
    }

    /// Visible key legend from the OEM keyboard layout.
    #[must_use]
    pub const fn label(self) -> &'static str {
        self.label
    }

    /// Index into the AULA per-key RGB table.
    #[must_use]
    pub const fn led_index(self) -> u8 {
        self.led_index
    }

    /// Slot in the captured wireless base matrix for this physical key.
    #[must_use]
    pub const fn matrix_slot(self) -> u8 {
        self.led_index
    }

    /// Left coordinate in the 600×231 OEM design space.
    #[must_use]
    pub const fn x(self) -> u16 {
        self.x
    }

    /// Top coordinate in the 600×231 OEM design space.
    #[must_use]
    pub const fn y(self) -> u16 {
        self.y
    }

    /// Key width in the OEM design space.
    #[must_use]
    pub const fn width(self) -> u16 {
        self.width
    }

    /// Key height in the OEM design space.
    #[must_use]
    pub const fn height(self) -> u16 {
        self.height
    }
}

/// The 87 keys and custom-RGB LED indexes from the installed F87 PRO layout.
///
/// The OEM layout's LED index matches the corresponding slot in the captured
/// wireless base-matrix read for all 87 keys. Physical RGB output at these
/// indexes has not yet been checked on an F87 PRO.
pub const AULA_F87_KEY_POSITIONS: [AulaKeyPosition; 87] = [
    AulaKeyPosition::new("Esc", 0, 17, 21, 24, 24),
    AulaKeyPosition::new("F1", 12, 82, 21, 24, 24),
    AulaKeyPosition::new("F2", 18, 115, 22, 24, 24),
    AulaKeyPosition::new("F3", 24, 147, 22, 24, 24),
    AulaKeyPosition::new("F4", 30, 180, 22, 24, 24),
    AulaKeyPosition::new("F5", 36, 228, 21, 24, 24),
    AulaKeyPosition::new("F6", 42, 260, 22, 24, 24),
    AulaKeyPosition::new("F7", 48, 294, 22, 24, 24),
    AulaKeyPosition::new("F8", 54, 325, 22, 24, 24),
    AulaKeyPosition::new("F9", 60, 374, 21, 24, 24),
    AulaKeyPosition::new("F10", 66, 407, 21, 24, 24),
    AulaKeyPosition::new("F11", 72, 439, 22, 24, 24),
    AulaKeyPosition::new("F12", 78, 471, 22, 24, 24),
    AulaKeyPosition::new("`", 1, 18, 59, 24, 24),
    AulaKeyPosition::new("1", 7, 50, 60, 24, 24),
    AulaKeyPosition::new("2", 13, 82, 60, 24, 24),
    AulaKeyPosition::new("3", 19, 114, 60, 24, 24),
    AulaKeyPosition::new("4", 25, 147, 60, 24, 24),
    AulaKeyPosition::new("5", 31, 179, 60, 24, 24),
    AulaKeyPosition::new("6", 37, 211, 60, 24, 24),
    AulaKeyPosition::new("7", 43, 244, 60, 24, 24),
    AulaKeyPosition::new("8", 49, 276, 60, 24, 24),
    AulaKeyPosition::new("9", 55, 310, 60, 24, 24),
    AulaKeyPosition::new("0", 61, 342, 60, 24, 24),
    AulaKeyPosition::new("-", 67, 374, 60, 24, 24),
    AulaKeyPosition::new("=", 73, 408, 60, 24, 24),
    AulaKeyPosition::new("Backspace", 79, 440, 60, 56, 24),
    AulaKeyPosition::new("PrtSc", 84, 508, 22, 24, 24),
    AulaKeyPosition::new("ScrLk", 90, 544, 21, 24, 24),
    AulaKeyPosition::new("Pause", 96, 576, 21, 24, 24),
    AulaKeyPosition::new("Tab", 2, 19, 91, 40, 24),
    AulaKeyPosition::new("Q", 8, 68, 91, 24, 24),
    AulaKeyPosition::new("W", 14, 99, 91, 24, 24),
    AulaKeyPosition::new("E", 20, 132, 91, 24, 24),
    AulaKeyPosition::new("R", 26, 164, 91, 24, 24),
    AulaKeyPosition::new("T", 32, 195, 91, 24, 24),
    AulaKeyPosition::new("Y", 38, 229, 91, 24, 24),
    AulaKeyPosition::new("U", 44, 261, 91, 24, 24),
    AulaKeyPosition::new("I", 50, 295, 91, 24, 24),
    AulaKeyPosition::new("O", 56, 326, 91, 24, 24),
    AulaKeyPosition::new("P", 62, 358, 91, 24, 24),
    AulaKeyPosition::new("[", 68, 390, 91, 24, 24),
    AulaKeyPosition::new("]", 74, 424, 91, 24, 24),
    AulaKeyPosition::new("\\", 80, 456, 91, 38, 24),
    AulaKeyPosition::new("Delete", 86, 509, 93, 24, 24),
    AulaKeyPosition::new("Insert", 85, 510, 60, 24, 24),
    AulaKeyPosition::new("Home", 91, 545, 60, 24, 24),
    AulaKeyPosition::new("PgUp", 97, 575, 60, 24, 24),
    AulaKeyPosition::new("CapsLock", 3, 19, 124, 46, 24),
    AulaKeyPosition::new("A", 9, 75, 124, 24, 24),
    AulaKeyPosition::new("S", 15, 108, 124, 24, 24),
    AulaKeyPosition::new("D", 21, 140, 124, 24, 24),
    AulaKeyPosition::new("F", 27, 172, 124, 24, 24),
    AulaKeyPosition::new("G", 33, 204, 124, 24, 24),
    AulaKeyPosition::new("H", 39, 237, 124, 24, 24),
    AulaKeyPosition::new("J", 45, 269, 124, 24, 24),
    AulaKeyPosition::new("K", 51, 301, 124, 24, 24),
    AulaKeyPosition::new("L", 57, 335, 124, 24, 24),
    AulaKeyPosition::new(";", 63, 367, 124, 24, 24),
    AulaKeyPosition::new("'", 69, 399, 124, 24, 24),
    AulaKeyPosition::new("Enter", 81, 431, 124, 64, 24),
    AulaKeyPosition::new("End", 92, 545, 93, 24, 24),
    AulaKeyPosition::new("PgDn", 98, 575, 93, 24, 24),
    AulaKeyPosition::new("LShift", 4, 18, 156, 64, 24),
    AulaKeyPosition::new("Z", 10, 91, 156, 24, 24),
    AulaKeyPosition::new("X", 16, 123, 156, 24, 24),
    AulaKeyPosition::new("C", 22, 155, 156, 24, 24),
    AulaKeyPosition::new("V", 28, 187, 156, 24, 24),
    AulaKeyPosition::new("B", 34, 221, 156, 24, 24),
    AulaKeyPosition::new("N", 40, 252, 156, 24, 24),
    AulaKeyPosition::new("M", 46, 285, 156, 24, 24),
    AulaKeyPosition::new(",", 52, 317, 156, 24, 24),
    AulaKeyPosition::new(".", 58, 351, 156, 24, 24),
    AulaKeyPosition::new("/", 64, 382, 156, 24, 24),
    AulaKeyPosition::new("RShift", 82, 417, 157, 80, 24),
    AulaKeyPosition::new("Up", 94, 541, 156, 24, 24),
    AulaKeyPosition::new("RCtrl", 83, 465, 188, 32, 24),
    AulaKeyPosition::new("LCtrl", 5, 19, 189, 32, 24),
    AulaKeyPosition::new("LWin", 11, 59, 189, 32, 24),
    AulaKeyPosition::new("LAlt", 17, 99, 189, 32, 24),
    AulaKeyPosition::new("Space", 35, 140, 189, 196, 24),
    AulaKeyPosition::new("RAlt", 53, 343, 189, 32, 24),
    AulaKeyPosition::new("FN", 59, 384, 189, 32, 24),
    AulaKeyPosition::new("App", 65, 425, 189, 32, 24),
    AulaKeyPosition::new("Left", 89, 510, 190, 24, 24),
    AulaKeyPosition::new("Down", 95, 543, 189, 24, 24),
    AulaKeyPosition::new("Right", 101, 575, 190, 24, 24),
];
