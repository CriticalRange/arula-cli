pub fn generate_arula_logo() -> String {
    r#"
  █████╗ ██████╗ ██╗   ██╗██╗      █████╗
 ██╔══██╗██╔══██╗██║   ██║██║     ██╔══██╗
 ███████║██████╔╝██║   ██║██║     ███████║
 ██╔══██║██╔══██╗██║   ██║██║     ██╔══██║
 ██║  ██║██║  ██║╚██████╔╝███████╗██║  ██║
 ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝ ╚══════╝╚═╝  ╚═╝

    Autonomous AI Command-Line Interface
    ⚡ Powered by Rust • Built with Ratatui
    Press ESC to open menu
"#.to_string()
}

pub fn generate_rust_crab() -> String {
    r#"🦀 Rust Crab ASCII Art:
     ⢀⣀⣀⣀⣀⣀⡀
    ⢸⣿⣿⣿⣿⣿⣿⡇
    ⢸⣿⡇⢀⣀⣀⣀⣿⣿⡇
    ⢸⣿⡇⢸⣿⣿⣿⣿⣿⡇
    ⢸⣿⣿⣿⣿⣿⣿⣿⡇
    ⢸⣿⣿⣿⣿⣿⣿⣿⡇
    ⢸⣿⣿⣿⣿⣿⣿⣿⡇
    ⢸⣿⣿⣿⣿⣿⣿⣿⡇
    ⠘⣿⣿⣿⣿⣿⣿⣿⡇
     ⠈⠻⣿⣿⣿⣿⠟
        ⠈⠻⠿⠟

The mighty Rust crab watches over your code! 🦀✨"#.to_string()
}

pub fn generate_fractal() -> String {
    r#"🌿 Recursive Fractal Generator:
         🦀
        /   \
       /_____\
      /       \
     /_________\
    /           \
   /_____________\
  /               \
 /_________________\
/                   \
/_____________________\

fn fractal(depth: u32) -> String {
    if depth == 0 { return "🦀".to_string(); }
    let parent = fractal(depth - 1);
    format!("  {}  \n /{}\\\n/_____\\\n", parent, parent)
}

Infinite recursion in Rust! 🌿"#.to_string()
}

pub fn generate_matrix() -> String {
    r#"💚 Matrix Digital Rain:
01110010 01110101 01110011 01110100  ▄▄▄▄▄▄▄
01000001 01010101 01010100 01001111  ▀█████
01101110 01101101 01101100 01110100  ▀█████
01001110 01000001 01001101 01001111  ▀█████
10110101 01010011 01000011 01001100  ▀█████

fn matrix_drops() {
    let drops = vec!["ARULA", "RUST", "SAFE", "FAST"];
    for drop in drops.iter().cycle() {
        println!("{:08b}", drop.as_bytes()[0]);
        thread::sleep(Duration::from_millis(50));
    }
}

💚 Wake up, Neo... The CLI has you..."#.to_string()
}

pub fn generate_demo() -> String {
    r#"🎨 Complete Art Demo:
🦀 RUST CRAB:
     ⢀⣀⣀⣀⣀⣀⡀
    ⢸⣿⣿⣿⣿⣿⣿⡇
    ⢸⣿⣿⣿⣿⣿⣿⡇
    ⢸⣿⣿⣿⣿⣿⣿⣿⡇
    ⠘⣿⣿⣿⣿⣿⣿⣿⡇
     ⠈⠻⣿⣿⣿⣿⠟

🌿 FRACTAL PATTERNS:
Recursive beauty in geometric forms

💚 MATRIX RAIN:
Digital code falling like rain
01110010 01110101 01110011 01110100

✨ All art styles completed!"#.to_string()
}