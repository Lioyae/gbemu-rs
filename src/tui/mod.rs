pub mod debug_view;
pub mod game_view;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_terminal_size_for_each_mode() {
        assert!(validate_size(162, 76, crate::app::ViewMode::Game).is_ok());
        assert!(validate_size(161, 76, crate::app::ViewMode::Game).is_err());
        assert!(validate_size(120, 36, crate::app::ViewMode::Debugger).is_ok());
        assert!(validate_size(119, 36, crate::app::ViewMode::Debugger).is_err());
    }
}
