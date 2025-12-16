use macroquad::prelude::*;

/// Parse a color string to a macroquad Color
/// Supports named colors like "RED", "BLUE", "GREEN", etc.
pub fn string_to_color(color_str: &str) -> Color {
    match color_str.to_uppercase().as_str() {
        "BLACK" => BLACK,
        "PINK" => PINK,
        "RED" => RED,
        "ORANGE" => ORANGE,
        "YELLOW" => YELLOW,
        "GREEN" => GREEN,
        "BLUE" => BLUE,
        "PURPLE" | "VIOLET" => VIOLET,
        "LIGHTGRAY" => LIGHTGRAY,
        "DARKGRAY" => DARKGRAY,
        "GRAY" | "GREY" => GRAY,
        "WHITE" => WHITE,
        _ => WHITE, // Default to WHITE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_to_color() {
        assert_eq!(string_to_color("RED"), RED);
        assert_eq!(string_to_color("red"), RED);
        assert_eq!(string_to_color("BLUE"), BLUE);
        assert_eq!(string_to_color("INVALID"), WHITE);
    }
}
