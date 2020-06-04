extern crate ncurses;
#[macro_use] extern crate const_cstr;

use core::ops::{BitOr, BitAnd};
use core::convert::TryInto;

mod imp_ncurses;

/// The set of modifier keys (e.g. Ctrl, Alt, and Shift) that were pressed at the time of an event.
/// Represented as an opaque bitmap to allow for extension with other keys, such as a Meta or
/// Command key.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Modifiers(u8);

impl BitOr for Modifiers {
    type Output = Modifiers;

    fn bitor(self, other: Modifiers) -> Modifiers {
        Modifiers(self.0 | other.0)
    }
}

impl BitAnd for Modifiers {
    type Output = Modifiers;

    fn bitand(self, other: Modifiers) -> Modifiers {
        Modifiers(self.0 & other.0)
    }
}

impl Modifiers {
    pub const NONE: Modifiers = Modifiers(0);

    pub const SHIFT: Modifiers = Modifiers(0b1);
    pub const ALT: Modifiers = Modifiers(0b10);
    pub const CTRL: Modifiers = Modifiers(0b100);

    pub const fn remove(self, other: Modifiers) -> Modifiers {
        Modifiers(self.0 & !other.0)
    }

    // Intrinsic const fn impls
    pub const fn bitor(self, other: Modifiers) -> Modifiers {
        Modifiers(self.0 | other.0)
    }

    pub const fn bitand(self, other: Modifiers) -> Modifiers {
        Modifiers(self.0 & other.0)
    }

    pub const fn eq(&self, other: &Modifiers) -> bool {
        self.0 == other.0
    }
}

/// A single event generated by a terminal. Simple text input, whether arriving via a pipe, a
/// paste command, or typing, will be represented with KeyPress events. These events are inherently
/// lossy and have different levels of support on different terminals. Depending on the use case,
/// certain modifier keys may just never be recorded, key repeats will be indistinguishable from
/// orignal presses, pastes may not be bracketed, and key releases may never be registered, among
/// other failures.
#[derive(Copy, Clone, Debug)]
pub enum Event {
    /// A single typing action by the user, input from stdin. Except between PasteBegin and PasteEnd
    /// events, these typically will not be control characters, as those are heuristically decoded
    /// into modifier keys combined with printable characters.
    KeyPress {
        modifiers: Modifiers,
        key: KeyInput,
        /// Whether this keypress comes from holding down a key
        is_repeat: bool,
    },
    /// This is kept as a separate event from KeyPress as it usually does not want to be handled in
    /// the same way and is supported by very few terminals, making it easy to miss in testing.
    KeyRelease {
        modifiers: Modifiers,
        key: KeyInput,
    },
    /// A motion or click of a mouse button. Modifiers typically are only be available on button
    /// state changes, not mouse motion.
    Mouse {
        device_id: u16,
        modifiers: Modifiers,
        buttons: ncurses::ll::mmask_t,
        x: u32,
        y: u32,
    },
    /// An indication that the following events occur purely as result of the user pasting from
    /// some unknown location that should be conservatively considered malicious. Applications
    /// should filter out control commands that happen during a paste, only considering the input
    /// as raw, unescaped text.
    PasteBegin,
    /// The marker indicating a return to normal user interaction.
    PasteEnd,
    /// The window has been resized and the application may want to rerender to fit the new sizee.
    Resize {
        width: u32,
        height: u32
    }
}

#[derive(Copy, Clone, Debug)]
pub enum KeyInput {
    Codepoint(char),
    /// A raw byte, not part of a unicode codepoint. This is generated when invalid UTF-8 is input.
    Byte(u8),
    /// A key not inputting a printable character.
    Special(i32),
}

pub struct InputStream<'a> {
    inner: imp_ncurses::InputStream,
    screen: ncurses::ll::WINDOW,
    // To prevent concurrency errors: we own all of stdin.
    _stdin_lock: std::io::StdinLock<'a>,
}

impl<'a> InputStream<'a> {
    pub unsafe fn init_with_ncurses(data: std::io::StdinLock<'a>, screen: ncurses::ll::WINDOW) -> InputStream<'a> {
        InputStream {
            inner: imp_ncurses::InputStream::init(screen),
            screen: screen,
            _stdin_lock: data
        }
    }

    // Wait until a new event is received. Note that the `Err` case should not generally be fatal;
    // this can be generated in some cases by inputs that terminal-input or ncurses is confused by.
    // In testing, this tends to happen when scrolling sideways on xterm, for example.
    pub fn next_event(&mut self) -> Result<Event, ()> {
        self.inner.next_event(self.screen)
    }

    // Set the time delay after an escape character is received to distinguish between the escape
    // key and automatic escape sequences.
    pub fn set_escdelay(&mut self, escdelay: core::time::Duration) {
        unsafe {
            ncurses::ll::set_escdelay(escdelay.as_millis().try_into().unwrap_or(i32::MAX));
        }
    }
}
