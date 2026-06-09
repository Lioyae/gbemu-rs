pub const SCREEN_WIDTH: usize = 160;
pub const SCREEN_HEIGHT: usize = 144;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Shade {
    #[default]
    White = 0,
    LightGray = 1,
    DarkGray = 2,
    Black = 3,
}

impl Shade {
    pub fn from_color(color: u8) -> Self {
        match color & 0x03 {
            0 => Self::White,
            1 => Self::LightGray,
            2 => Self::DarkGray,
            3 => Self::Black,
            _ => unreachable!("颜色值已经限制为两位"),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Framebuffer {
    pixels: [Shade; SCREEN_WIDTH * SCREEN_HEIGHT],
}

impl Framebuffer {
    pub fn new() -> Self {
        Self {
            pixels: [Shade::White; SCREEN_WIDTH * SCREEN_HEIGHT],
        }
    }

    pub fn width(&self) -> usize {
        SCREEN_WIDTH
    }

    pub fn height(&self) -> usize {
        SCREEN_HEIGHT
    }

    pub fn pixels(&self) -> &[Shade] {
        &self.pixels
    }

    pub fn pixel(&self, x: usize, y: usize) -> Shade {
        self.pixels
            .get(y.saturating_mul(SCREEN_WIDTH).saturating_add(x))
            .copied()
            .filter(|_| x < SCREEN_WIDTH && y < SCREEN_HEIGHT)
            .unwrap_or(Shade::White)
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, shade: Shade) {
        if x < SCREEN_WIDTH && y < SCREEN_HEIGHT {
            self.pixels[y * SCREEN_WIDTH + x] = shade;
        }
    }

    pub fn clear(&mut self, shade: Shade) {
        self.pixels.fill(shade);
    }
}

impl Default for Framebuffer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_with_white_pixels() {
        let framebuffer = Framebuffer::new();

        assert_eq!(framebuffer.width(), 160);
        assert_eq!(framebuffer.height(), 144);
        assert_eq!(framebuffer.pixel(0, 0), Shade::White);
        assert_eq!(framebuffer.pixel(159, 143), Shade::White);
    }

    #[test]
    fn writes_pixels_inside_bounds_and_ignores_outside_bounds() {
        let mut framebuffer = Framebuffer::new();

        framebuffer.set_pixel(10, 20, Shade::DarkGray);
        framebuffer.set_pixel(160, 20, Shade::Black);
        framebuffer.set_pixel(10, 144, Shade::Black);

        assert_eq!(framebuffer.pixel(10, 20), Shade::DarkGray);
        assert_eq!(framebuffer.pixel(159, 20), Shade::White);
    }

    #[test]
    fn shade_converts_from_two_bit_color() {
        assert_eq!(Shade::from_color(0), Shade::White);
        assert_eq!(Shade::from_color(1), Shade::LightGray);
        assert_eq!(Shade::from_color(2), Shade::DarkGray);
        assert_eq!(Shade::from_color(3), Shade::Black);
        assert_eq!(Shade::from_color(7), Shade::Black);
    }
}
