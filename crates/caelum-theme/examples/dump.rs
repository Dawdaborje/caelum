// SPDX-License-Identifier: GPL-3.0-or-later
//! Prints the dark and light schemes for a wallpaper as hex: `dump <image>`.
fn main() {
    let path = std::env::args().nth(1).expect("usage: dump <image>");
    for dark in [true, false] {
        let scheme = caelum_theme::scheme_from_image(path.as_ref(), dark).expect("wallpaper");
        println!("## {}\n", if dark { "Dark" } else { "Light" });
        for (name, caelum_theme::Rgba(r, g, b, _)) in scheme.roles() {
            println!("- `{name}`: `#{r:02x}{g:02x}{b:02x}`");
        }
        println!();
    }
}
