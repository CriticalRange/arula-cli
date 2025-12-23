use arula_desktop::theme::{ThemeMode, palette_from_mode};

fn main() {
    println!("Theme Colors:");
    for mode in [ThemeMode::Light, ThemeMode::Dark, ThemeMode::Black] {
        let pal = palette_from_mode(mode);
        println!("\n{:?} Theme:", mode);
        println!("  Background: {:?}", pal.background);
        println!("  Accent: {:?}", pal.accent);
        println!("  Text: {:?}", pal.text);
    }
}
