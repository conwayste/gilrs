use std::error::Error as StdError;
use std::fmt::{self, Display};

use uuid::Uuid;

use gamepad::{Axis, Button};

// Must be sorted!
static BUTTONS_SDL: [&'static str; 19] = [
    "a", "b", "back", "c", "dpdown", "dpleft", "dpright", "dpup", "guide", "leftshoulder",
    "leftstick", "lefttrigger", "rightshoulder", "rightstick", "righttrigger", "start", "x", "y",
    "z",
];
static BUTTONS: [Button; 19] = [
    Button::South, Button::East, Button::Select, Button::C, Button::DPadDown, Button::DPadLeft,
    Button::DPadRight, Button::DPadUp, Button::Mode, Button::LeftTrigger, Button::LeftThumb,
    Button::LeftTrigger2, Button::RightTrigger, Button::RightThumb, Button::RightTrigger2,
    Button::Start, Button::West, Button::North, Button::Z,
];

// Must be sorted!
static AXES_SDL: [&'static str; 10] = [
    "leftshoulder", "lefttrigger", "leftx", "lefty", "leftz", "rightshoulder", "righttrigger",
    "rightx", "righty", "rightz",
];
static AXES: [Axis; 10] = [
    Axis::LeftTrigger, Axis::LeftTrigger2, Axis::LeftStickX, Axis::LeftStickY, Axis::LeftZ,
    Axis::RightTrigger, Axis::RightTrigger2, Axis::RightStickX, Axis::RightStickY, Axis::RightZ,
];

pub struct Parser<'a> {
    data: &'a str,
    pos: usize,
    state: State,
}

impl<'a> Parser<'a> {
    pub fn new(mapping: &'a str) -> Self {
        Parser {
            data: mapping,
            pos: 0,
            state: State::Uuid,
        }
    }

    pub fn next_token(&mut self) -> Option<Result<Token, Error>> {
        if self.pos >= self.data.len() {
            None
        } else {
            Some(match self.state {
                State::Uuid => self.parse_uuid(),
                State::Name => self.parse_name(),
                State::KeyVal => self.parse_key_val(),
                State::Invalid => Err(Error::new(ErrorKind::InvalidParserState, self.pos)),
            })
        }
    }

    fn parse_uuid(&mut self) -> Result<Token, Error> {
        let next_comma = self.next_comma_or_end();
        let uuid = Uuid::parse_str(&self.data[self.pos..next_comma])
            .map(|uuid| Token::Uuid(uuid))
            .or(Err(Error::new(ErrorKind::InvalidGuid, self.pos)));

        if uuid.is_err() {
            self.state = State::Invalid;
        } else if next_comma == self.data.len() {
            self.state = State::Invalid;

            return Err(Error::new(ErrorKind::UnexpectedEnd, self.pos));
        } else {
            self.state = State::Name;
            self.pos = next_comma + 1;
        }

        uuid
    }

    fn parse_name(&mut self) -> Result<Token, Error> {
        let next_comma = self.next_comma_or_end();
        let name = &self.data[self.pos..next_comma];

        self.state = State::KeyVal;
        self.pos = next_comma + 1;

        Ok(Token::Name(name))
    }

    fn parse_key_val(&mut self) -> Result<Token, Error> {
        let next_comma = self.next_comma_or_end();
        let pair = &self.data[self.pos..next_comma];
        let pos = self.pos;
        self.pos = next_comma + 1;

        let mut split = pair.split(':');
        let key = split
            .next()
            .ok_or(Error::new(ErrorKind::InvalidKeyValPair, pos))?;
        let value = split
            .next()
            .ok_or(Error::new(ErrorKind::InvalidKeyValPair, pos))?;

        if split.next().is_some() {
            return Err(Error::new(ErrorKind::InvalidKeyValPair, pos));
        }

        if key == "platform" {
            return Ok(Token::Platform(value));
        }

        let mut input = AxisRange::Full;
        let mut output = AxisRange::Full;
        let mut inverted = false;
        let mut is_axis = false;

        let from = match value.get(0..1) {
            Some("+") if value.get(1..2) == Some("a") => {
                is_axis = true;
                input = AxisRange::UpperHalf;

                if value.get((value.len() - 1)..) == Some("~") {
                    inverted = true;

                    &value[2..(value.len() - 1)]
                } else {
                    &value[2..]
                }
            }
            Some("-") if value.get(1..2) == Some("a") => {
                is_axis = true;
                input = AxisRange::LowerHalf;

                if value.get((value.len() - 1)..) == Some("~") {
                    inverted = true;

                    &value[2..(value.len() - 1)]
                } else {
                    &value[2..]
                }
            }
            Some("a") => {
                is_axis = true;

                if value.get((value.len() - 1)..) == Some("~") {
                    inverted = true;

                    &value[1..(value.len() - 1)]
                } else {
                    &value[1..]
                }
            }
            Some("b") => &value[1..],
            Some("h") => {
                let dot_idx = value
                    .find('.')
                    .ok_or(Error::new(ErrorKind::InvalidValue, pos))?;
                let hat = value[1..dot_idx]
                    .parse()
                    .or(Err(Error::new(ErrorKind::InvalidValue, pos + 1)))?;
                let direction = value
                    .get((dot_idx as usize + 1)..)
                    .and_then(|s| s.parse().ok())
                    .ok_or(Error::new(ErrorKind::InvalidValue, pos + dot_idx + 1))?;

                let idx = BUTTONS_SDL
                    .binary_search(&key)
                    .or(Err(Error::new(ErrorKind::UnknownButton, pos)))?;

                return Ok(Token::HatMapping { hat, direction, to: BUTTONS[idx] });
            }
            _ => return Err(Error::new(ErrorKind::InvalidValue, pos)),
        }.parse::<u16>()
            .or(Err(Error::new(ErrorKind::InvalidValue, pos)))?;

        if is_axis {
            let key = match key.get(0..1) {
                Some("+") => {
                    output = AxisRange::UpperHalf;

                    &key[1..]
                }
                Some("-") => {
                    output = AxisRange::LowerHalf;

                    &key[1..]
                }
                _ => key,
            };

            let idx = AXES_SDL
                .binary_search(&key)
                .or(Err(Error::new(ErrorKind::UnknownAxis, pos)))?;

            Ok(Token::AxisMapping {
                from,
                to: AXES[idx],
                input,
                output,
                inverted,
            })
        } else {
            let idx = BUTTONS_SDL
                .binary_search(&key)
                .or(Err(Error::new(ErrorKind::UnknownButton, pos)))?;

            Ok(Token::ButtonMapping { from, to: BUTTONS[idx] })
        }
    }

    fn next_comma_or_end(&self) -> usize {
        self.data[self.pos..]
            .find(',')
            .map(|x| x + self.pos)
            .unwrap_or(self.data.len())
    }
}

pub enum Token<'a> {
    Uuid(Uuid),
    Platform(&'a str),
    Name(&'a str),
    AxisMapping {
        from: u16,
        to: Axis,
        input: AxisRange,
        output: AxisRange,
        inverted: bool,
    },
    ButtonMapping { from: u16, to: Button },
    // This is just SDL representation, we will convert this to axis mapping later
    HatMapping {
        hat: u16,
        // ?
        direction: u16,
        to: Button,
    },
}

#[repr(u8)]
pub enum AxisRange {
    LowerHalf,
    UpperHalf,
    Full,
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum State {
    Uuid,
    Name,
    KeyVal,
    Invalid,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    position: usize,
    kind: ErrorKind,
}

impl Error {
    pub fn new(kind: ErrorKind, position: usize) -> Self {
        Error { position, kind }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorKind {
    InvalidGuid,
    InvalidKeyValPair,
    InvalidValue,
    UnknownAxis,
    UnknownButton,
    InvalidParserState,
    UnexpectedEnd,
}

impl StdError for Error {
    fn description(&self) -> &str {
        match self.kind {
            ErrorKind::InvalidGuid => "GUID is invalid",
            ErrorKind::InvalidKeyValPair => "expected key value pair",
            ErrorKind::InvalidValue => "value is not valid",
            ErrorKind::UnknownAxis => "invalid axis name",
            ErrorKind::UnknownButton => "invalid button name",
            ErrorKind::InvalidParserState => "attempt to parse after unrecoverable error",
            ErrorKind::UnexpectedEnd => "mapping does not have all required fields",
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_fmt(format_args!("{} at {}", self.description(), self.position))
    }
}