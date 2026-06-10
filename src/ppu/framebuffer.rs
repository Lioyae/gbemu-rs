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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pixel {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl Pixel {
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    pub fn from_rgb555(value: u16) -> Self {
        fn expand(value: u16) -> u8 {
            let value = value as u8 & 0x1f;
            (value << 3) | (value >> 2)
        }

        Self::rgb(expand(value), expand(value >> 5), expand(value >> 10))
    }
}

impl From<Shade> for Pixel {
    fn from(shade: Shade) -> Self {
        match shade {
            Shade::White => Self::rgb(224, 248, 208),
            Shade::LightGray => Self::rgb(136, 192, 112),
            Shade::DarkGray => Self::rgb(52, 104, 86),
            Shade::Black => Self::rgb(8, 24, 32),
        }
    }
}

impl PartialEq<Shade> for Pixel {
    fn eq(&self, other: &Shade) -> bool {
        *self == Pixel::from(*other)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Framebuffer {
    pixels: [Pixel; SCREEN_WIDTH * SCREEN_HEIGHT],
}

impl Framebuffer {
    pub fn new() -> Self {
        Self {
            pixels: [Pixel::rgb(224, 248, 208); SCREEN_WIDTH * SCREEN_HEIGHT],
        }
    }

    pub fn width(&self) -> usize {
        SCREEN_WIDTH
    }

    pub fn height(&self) -> usize {
        SCREEN_HEIGHT
    }

    pub fn pixels(&self) -> &[Pixel] {
        &self.pixels
    }

    pub fn pixel(&self, x: usize, y: usize) -> Pixel {
        self.pixels
            .get(y.saturating_mul(SCREEN_WIDTH).saturating_add(x))
            .copied()
            .filter(|_| x < SCREEN_WIDTH && y < SCREEN_HEIGHT)
            .unwrap_or_else(|| Pixel::from(Shade::White))
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, pixel: impl Into<Pixel>) {
        if x < SCREEN_WIDTH && y < SCREEN_HEIGHT {
            self.pixels[y * SCREEN_WIDTH + x] = pixel.into();
        }
    }

    pub fn clear(&mut self, pixel: impl Into<Pixel>) {
        self.pixels.fill(pixel.into());
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

    #[test]
    fn converts_cgb_rgb555_to_eight_bit_rgb() {
        assert_eq!(Pixel::from_rgb555(0x001f), Pixel::rgb(255, 0, 0));
        assert_eq!(Pixel::from_rgb555(0x03e0), Pixel::rgb(0, 255, 0));
        assert_eq!(Pixel::from_rgb555(0x7c00), Pixel::rgb(0, 0, 255));
        assert_eq!(Pixel::from_rgb555(0x7fff), Pixel::rgb(255, 255, 255));
    }
}
