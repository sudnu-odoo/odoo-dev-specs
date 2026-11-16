/// Named colours, so the assistant does not have to pick hex codes.
/// Values are Excalidraw's own palette (light fill, strong stroke).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Gray,
    Blue,
    Green,
    Yellow,
    Red,
    Purple,
}

impl Color {
    pub const NAMES: [&'static str; 6] = ["gray", "blue", "green", "yellow", "red", "purple"];

    pub fn parse(name: &str) -> Option<Color> {
        match name.trim().to_lowercase().as_str() {
            "gray" | "grey" => Some(Color::Gray),
            "blue" => Some(Color::Blue),
            "green" => Some(Color::Green),
            "yellow" => Some(Color::Yellow),
            "red" => Some(Color::Red),
            "purple" => Some(Color::Purple),
            _ => None,
        }
    }

    pub(crate) fn fill(self) -> &'static str {
        match self {
            Color::Gray => "#e9ecef",
            Color::Blue => "#a5d8ff",
            Color::Green => "#b2f2bb",
            Color::Yellow => "#ffec99",
            Color::Red => "#ffc9c9",
            Color::Purple => "#d0bfff",
        }
    }

    pub(crate) fn stroke(self) -> &'static str {
        match self {
            Color::Gray => "#495057",
            Color::Blue => "#1971c2",
            Color::Green => "#2f9e44",
            Color::Yellow => "#f08c00",
            Color::Red => "#e03131",
            Color::Purple => "#6741d9",
        }
    }
}
