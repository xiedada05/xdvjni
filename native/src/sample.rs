//! Hand-crafted XDV sample used by both the Rust and the Java test suites.
//!
//! The byte-level layout follows the XDV format as parsed by `tectonic_xdv`
//! (preamble id 7, unit 25400000/473628672). The event sequence produced by
//! this sample is asserted in `native/tests/sample_test.rs` and mirrored by
//! the JUnit tests, so any change here must be reflected there as well.

/// Big-endian byte builder for XDV structures.
#[derive(Default)]
pub struct Builder {
    buf: Vec<u8>,
}

impl Builder {
    pub fn pos(&self) -> usize {
        self.buf.len()
    }

    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    pub fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }

    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }

    pub fn i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_be_bytes());
    }

    /// A length-prefixed (`u8`) byte string.
    pub fn u8_str(&mut self, s: &str) {
        let bytes = s.as_bytes();
        assert!(bytes.len() <= 255, "string too long for u8 length prefix");
        self.u8(bytes.len() as u8);
        self.buf.extend_from_slice(bytes);
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buf
    }
}

/// Generate the complete sample XDV file.
///
/// Expected event sequence (see the tests):
///
/// * header: XDV, comment `"xdvjni sample"`
/// * page 1 (`bop` counters `[1, 0, ...]`, `previous_bop = -1`)
///   - native font `"NotoSerif"` (num 5) with `color_rgba = 0xFF8051A0`
///   - after `down4(100)`: `v = 100`
///   - glyph run at `h = 0`, width 655360, glyphs `[36, 37]`,
///     `x = [0, 327680]`, `y = [100, 100]`
///   - special `"color=red"` at `(655360, 100)`
///   - text `"Hello"` + glyphs `[40, 41]`, width 327680,
///     `x = [655360, 720896]`, `y = [100, 100]`
///   - `set_rule` at `(983040, 100)`, height 200, width 400
///   - char run `[72 ('H'), 105 ('i')]` in font 5
/// * page 2 (counters `[2, 0, ...]`, `previous_bop = 28`)
///   - native font `"NotoSansCJK"` (num 6) with `extend = 0x10000` and
///     `embolden = 0x8000`
///   - `put_rule` at `(0, 0)`, height 50, width 50
///   - char run `[88 ('X')]` in font 6
pub fn generate_sample() -> Vec<u8> {
    let mut b = Builder::default();

    // Preamble: op 247, XDV id byte 7.
    b.u8(247);
    b.u8(7);
    b.u32(25_400_000);
    b.u32(473_628_672);
    b.u32(1000); // mag
    b.u8_str("xdvjni sample");

    // ---- Page 1 ----
    let bop1 = b.pos();
    b.u8(139); // bop
    for &c in &[1, 0, 0, 0, 0, 0, 0, 0, 0, 0] {
        b.i32(c);
    }
    b.i32(-1); // previous_bop

    // Native font 5 "NotoSerif", colored.
    b.u8(252);
    b.i32(5); // font_num
    b.i32(655_360); // size (fixed-point)
    b.u16(0x0200); // Colored
    b.u8_str("NotoSerif");
    b.u32(0); // face_index
    b.u32(0xFF80_51A0); // color_rgba

    b.u8(171 + 5); // set font 5

    b.u8(160); // down4
    b.i32(100); // v = 100

    // Glyph run: width 655360, glyphs [36, 37] at (h+0, v+0) and
    // (h+327680, v+0). SET_GLYPHS encodes one (dx, dy) pair per glyph,
    // followed by the glyph ids.
    b.u8(253);
    b.i32(655_360);
    b.u16(2);
    b.i32(0); // glyph 0 dx
    b.i32(0); // glyph 0 dy
    b.i32(327_680); // glyph 1 dx
    b.i32(0); // glyph 1 dy
    b.u16(36);
    b.u16(37);
    // h = 655360

    b.u8(239); // special1
    b.u8_str("color=red");

    // Text + glyphs: "Hello", width 327680, glyphs [40, 41].
    // Layout: n_chars u16, chars u16*, width i32, n_glyphs u16,
    // then one (dx, dy) pair per glyph, then the glyph ids.
    b.u8(254);
    b.u16(5);
    for ch in "Hello".encode_utf16() {
        b.u16(ch);
    }
    b.i32(327_680);
    b.u16(2);
    b.i32(0); // glyph 0 dx
    b.i32(0); // glyph 0 dy
    b.i32(65_536); // glyph 1 dx
    b.i32(0); // glyph 1 dy
    b.u16(40);
    b.u16(41);
    // h = 983040

    // set_rule (opcode 132): height 200, width 400 (moves h afterwards).
    b.u8(132);
    b.i32(200);
    b.i32(400);

    // Char run 'H' 'i', flushed by eop.
    b.u8(72);
    b.u8(105);

    b.u8(140); // eop

    // ---- Page 2 ----
    let bop2 = b.pos();
    b.u8(139); // bop
    for &c in &[2, 0, 0, 0, 0, 0, 0, 0, 0, 0] {
        b.i32(c);
    }
    b.i32(bop1 as i32); // previous_bop

    // Native font 6 "NotoSansCJK" with extend + embolden.
    b.u8(252);
    b.i32(6); // font_num
    b.i32(300_000); // size
    b.u16(0x1000 | 0x4000); // Extend | Embolden
    b.u8_str("NotoSansCJK");
    b.u32(2); // face_index
    b.u32(0x1_0000); // extend
    b.u32(0x8000); // embolden

    b.u8(171 + 6); // set font 6

    // put_rule (opcode 137) at (0, 0): height 50, width 50 (does not move
    // the current point).
    b.u8(137);
    b.i32(50);
    b.i32(50);

    b.u8(88); // 'X'
    b.u8(140); // eop

    // ---- Postamble ----
    let post = b.pos();
    b.u8(248); // post
    b.u32(bop2 as u32); // last_bop
    b.u32(25_400_000);
    b.u32(473_628_672);
    b.u32(1000); // mag
    b.u32(100_000); // tallest page
    b.u32(100_000); // widest page
    b.u16(1); // max stack depth
    b.u16(2); // page count

    b.u8(249); // post_post
    b.u32(post as u32); // pointer to the postamble
    b.u8(7); // XDV id byte
    for _ in 0..5 {
        b.u8(0xDF);
    }

    b.into_bytes()
}
