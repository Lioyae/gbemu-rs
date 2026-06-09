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
