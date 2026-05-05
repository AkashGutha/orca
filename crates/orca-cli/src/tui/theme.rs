use ratatui::style::Color;

pub(super) const MAX_VISIBLE_PANES: usize = 4;
pub(super) const LARGE_HEADER_HEIGHT: u16 = 9;
pub(super) const COMPACT_HEADER_HEIGHT: u16 = 3;
pub(super) const MIN_LARGE_HEADER_AREA_HEIGHT: u16 = 16;
pub(super) const ORCA_RED: Color = Color::Rgb(255, 48, 0);
pub(super) const ORCA_ORANGE: Color = Color::Rgb(255, 81, 0);
pub(super) const ORCA_AMBER: Color = Color::Rgb(255, 126, 0);
pub(super) const ORCA_GOLD: Color = Color::Rgb(255, 205, 0);
pub(super) const ORCA_LIGHT_GOLD: Color = Color::Rgb(255, 224, 102);
pub(super) const ORCA_BANNER_LINES: [&str; 6] = [
    " ██████╗ ██████╗  ██████╗ █████╗ ",
    "██╔═══██╗██╔══██╗██╔════╝██╔══██╗",
    "██║   ██║██████╔╝██║     ███████║",
    "██║   ██║██╔══██╗██║     ██╔══██║",
    "╚██████╔╝██║  ██║╚██████╗██║  ██║",
    " ╚═════╝ ╚═╝  ╚═╝ ╚═════╝╚═╝  ╚═╝",
];
