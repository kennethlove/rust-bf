pub mod catppuccin {
    use nu_ansi_term::Color;
    pub struct Mocha;
    impl Mocha {
        // Base colors
        pub const TEXT: Color = Color::Rgb(205, 214, 244);  // Text
        pub const SURFACE2: Color = Color::Rgb(108, 112, 134);  // Subtle dim

        // Accents
        pub const RED: Color = Color::Rgb(243, 139, 168);
        pub const GREEN: Color = Color::Rgb(166, 227, 161);
        pub const YELLOW: Color = Color::Rgb(249, 226, 175);
        pub const BLUE: Color = Color::Rgb(137, 180, 250);
        pub const MAUVE: Color = Color::Rgb(203, 166, 247);
        pub const PEACH: Color = Color::Rgb(250, 179, 135);
        pub const TEAL: Color = Color::Rgb(148, 226, 213);
        pub const SKY: Color = Color::Rgb(137, 220, 235);
    }
}
