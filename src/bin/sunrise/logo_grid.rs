// The Sunrise mark's pixels, shared by the window and the build script that
// writes the .exe icon: one row per line, each character is a palette index.
// Kept free of any dependency so `include!` works in both places.

const LOGO_W: usize = 16;
const LOGO_H: usize = 16;

/// Index 0 is transparent; the rest are RGB.
const LOGO_PALETTE: [(u8, u8, u8); 10] = [
    (0, 0, 0),       // 0 transparent
    (5, 7, 11),      // 1 ground shadow
    (16, 19, 30),    // 2 night sky
    (38, 24, 32),    // 3 dusk sky
    (107, 58, 63),   // 4 far dune
    (51, 24, 30),    // 5 near dune
    (255, 158, 59),  // 6 amber (reserved)
    (255, 128, 20),  // 7 sun rim
    (255, 174, 77),  // 8 sun
    (255, 229, 180), // 9 sun core
];

const LOGO_ART: [&str; LOGO_H] = [
    "2222222222222222",
    "2222222222222222",
    "2222222222222222",
    "2222227887222222",
    "2222788888872222",
    "3337999999997333",
    "3379999999999733",
    "3379999999999733",
    "3378888888888733",
    "3337888888877333",
    "5555555444444444",
    "5555555555555555",
    "5555555555555555",
    "5555555555555555",
    "1111111111111111",
    "1111111111111111",
];
