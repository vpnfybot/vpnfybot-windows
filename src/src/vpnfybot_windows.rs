#![windows_subsystem = "windows"]

#[path = "gui_rfd.rs"]
mod gui_rfd;

fn main() -> eframe::Result<()> {
	gui_rfd::app_main()
}