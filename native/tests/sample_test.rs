//! Validates the hand-crafted sample XDV against the vendored XDV parser
//! (upstream `tectonic_xdv`, patched for legacy fonts): both the byte layout
//! produced by `xdvjni::sample` and the exact event sequence (which the Java
//! tests mirror).

use std::fmt;
use std::fmt::Write as _;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use xdvjni::{FileType, XdvError, XdvEvents, XdvParser};

/// Error type for the test recorder.
///
/// `XdvParser::process` requires `From<io::Error>` in addition to
/// `From<XdvError>`, and neither can be implemented for `String` (the
/// reflexive `From<T> for T` impl rules it out), hence the wrapper.
#[derive(Debug)]
struct TestError(String);

impl From<std::io::Error> for TestError {
    fn from(e: std::io::Error) -> Self {
        TestError(e.to_string())
    }
}

impl From<XdvError> for TestError {
    fn from(e: XdvError) -> Self {
        TestError(e.to_string())
    }
}

impl fmt::Display for TestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for TestError {}

#[derive(Debug, PartialEq)]
enum Event {
    Header(FileType, Vec<u8>),
    BeginPage(Vec<i32>, i32),
    Special(i32, i32, Vec<u8>),
    CharRun(i32, Vec<i32>, Vec<i32>, Vec<i32>),
    DefineFont(String, String, i32, u32, u32, u32),
    GlyphRun(i32, Vec<u16>, Vec<i32>, Vec<i32>),
    TextAndGlyphs(i32, String, i32, Vec<u16>, Vec<i32>, Vec<i32>),
    DefineNativeFont(
        String,
        i32,
        i32,
        u32,
        Option<u32>,
        Option<u32>,
        Option<u32>,
        Option<u32>,
    ),
    Rule(i32, i32, i32, i32),
}

#[derive(Debug, Default)]
struct Recorder {
    events: Vec<Event>,
}

impl XdvEvents for Recorder {
    type Error = TestError;

    fn handle_header(&mut self, filetype: FileType, comment: &[u8]) -> Result<(), TestError> {
        self.events.push(Event::Header(filetype, comment.to_vec()));
        Ok(())
    }

    fn handle_begin_page(&mut self, counters: &[i32], previous_bop: i32) -> Result<(), TestError> {
        self.events
            .push(Event::BeginPage(counters.to_vec(), previous_bop));
        Ok(())
    }

    fn handle_special(&mut self, x: i32, y: i32, contents: &[u8]) -> Result<(), TestError> {
        self.events
            .push(Event::Special(x, y, contents.to_vec()));
        Ok(())
    }

    fn handle_char_run(
        &mut self,
        font_num: i32,
        chars: &[i32],
        xs: &[i32],
        ys: &[i32],
    ) -> Result<(), TestError> {
        self.events
            .push(Event::CharRun(font_num, chars.to_vec(), xs.to_vec(), ys.to_vec()));
        Ok(())
    }

    fn handle_define_font(
        &mut self,
        area: &str,
        name: &str,
        font_num: i32,
        checksum: u32,
        scale_factor: u32,
        design_size: u32,
    ) -> Result<(), TestError> {
        self.events.push(Event::DefineFont(
            area.to_string(),
            name.to_string(),
            font_num,
            checksum,
            scale_factor,
            design_size,
        ));
        Ok(())
    }

    fn handle_glyph_run(
        &mut self,
        font_num: i32,
        glyphs: &[u16],
        x: &[i32],
        y: &[i32],
    ) -> Result<(), TestError> {
        self.events.push(Event::GlyphRun(
            font_num,
            glyphs.to_vec(),
            x.to_vec(),
            y.to_vec(),
        ));
        Ok(())
    }

    fn handle_text_and_glyphs(
        &mut self,
        font_num: i32,
        text: &str,
        width: i32,
        glyphs: &[u16],
        x: &[i32],
        y: &[i32],
    ) -> Result<(), TestError> {
        self.events.push(Event::TextAndGlyphs(
            font_num,
            text.to_string(),
            width,
            glyphs.to_vec(),
            x.to_vec(),
            y.to_vec(),
        ));
        Ok(())
    }

    fn handle_define_native_font(
        &mut self,
        name: &str,
        font_num: i32,
        size: i32,
        face_index: u32,
        color_rgba: Option<u32>,
        extend: Option<u32>,
        slant: Option<u32>,
        embolden: Option<u32>,
    ) -> Result<(), TestError> {
        self.events.push(Event::DefineNativeFont(
            name.to_string(),
            font_num,
            size,
            face_index,
            color_rgba,
            extend,
            slant,
            embolden,
        ));
        Ok(())
    }

    fn handle_rule(&mut self, x: i32, y: i32, height: i32, width: i32) -> Result<(), TestError> {
        self.events.push(Event::Rule(x, y, height, width));
        Ok(())
    }
}

fn counters(first: i32) -> Vec<i32> {
    let mut c = vec![0; 10];
    c[0] = first;
    c
}

#[test]
fn sample_produces_expected_events() {
    let data = xdvjni::sample::generate_sample();
    let recorder = Recorder::default();
    let (recorder, n) = XdvParser::process(std::io::Cursor::new(&data), recorder).unwrap();
    assert_eq!(n, data.len() as u64, "all bytes must be consumed");

    let expected = vec![
        Event::Header(FileType::Xdv, b"xdvjni sample".to_vec()),
        Event::BeginPage(counters(1), -1),
        Event::DefineNativeFont(
            "NotoSerif".to_string(),
            5,
            655_360,
            0,
            Some(0xFF80_51A0),
            None,
            None,
            None,
        ),
        Event::GlyphRun(5, vec![36, 37], vec![0, 327_680], vec![100, 100]),
        Event::Special(655_360, 100, b"color=red".to_vec()),
        Event::TextAndGlyphs(
            5,
            "Hello".to_string(),
            327_680,
            vec![40, 41],
            vec![655_360, 720_896],
            vec![100, 100],
        ),
        Event::Rule(983_040, 100, 200, 400),
        Event::CharRun(5, vec![72, 105], vec![983_440, 983_440], vec![100, 100]),
        Event::BeginPage(counters(2), 28),
        Event::DefineNativeFont(
            "NotoSansCJK".to_string(),
            6,
            300_000,
            2,
            None,
            Some(0x1_0000),
            None,
            Some(0x8000),
        ),
        Event::Rule(0, 0, 50, 50),
        Event::CharRun(6, vec![88], vec![0], vec![0]),
    ];
    assert_eq!(recorder.events, expected);
}

/// Legacy `fnt_def` + `set_char` stream with injected TFM widths: the
/// definition must be forwarded and each character must advance the current
/// point by its injected width, so the trailing `set_rule` lands after the
/// text instead of on top of it.
#[test]
fn legacy_font_def_and_width_injection() {
    /// Build a minimal one-page XDV with an optional legacy fnt_def.
    fn build(with_font_def: bool) -> Vec<u8> {
        let mut b = xdvjni::sample::Builder::default();
        b.u8(247); // pre
        b.u8(7); // XDV id byte
        b.u32(25_400_000);
        b.u32(473_628_672);
        b.u32(1000); // mag
        b.u8_str("legacy width injection");

        b.u8(139); // bop
        for &c in &[1, 0, 0, 0, 0, 0, 0, 0, 0, 0] {
            b.i32(c);
        }
        b.i32(-1);

        if with_font_def {
            b.u8(243); // fnt_def1, font_num 5
            b.u8(5);
            b.u32(0xDEAD_BEEF); // checksum
            b.u32(655_360); // scale_factor (DVI units)
            b.u32(655_360); // design_size
            b.u8_str(""); // area
            b.u8_str("pplr8t"); // name
        }

        b.u8(171 + 5); // fnt num 5
        b.u8(72); // 'H'
        b.u8(105); // 'i'
        b.u8(132); // set_rule h=30 w=100
        b.i32(30);
        b.i32(100);

        b.u8(140); // eop

        // Preamble length: op + id + num + den + mag + n_comment + comment.
        let pre_len =
            1 + 1 + 4 + 4 + 4 + 1 + "legacy width injection".len();
        let bop_pos = pre_len as u32;

        let mut t = xdvjni::sample::Builder::default();
        t.u8(248); // post
        t.u32(bop_pos);
        t.u32(25_400_000);
        t.u32(473_628_672);
        t.u32(1000);
        t.u32(100_000);
        t.u32(100_000);
        t.u16(1);
        t.u16(1);
        let mut bytes = b.into_bytes();
        let post_pos = bytes.len() as u32;
        bytes.extend_from_slice(&t.into_bytes());

        let mut p = xdvjni::sample::Builder::default();
        p.u8(249); // post_post
        p.u32(post_pos);
        p.u8(7);
        for _ in 0..5 {
            p.u8(0xDF);
        }
        bytes.extend_from_slice(&p.into_bytes());
        bytes
    }

    // Slot-indexed width table: 'H' = 409_600, 'i' = 278_528, rest 0.
    let table = {
        let mut w = vec![0i64; 256];
        w[72] = 409_600;
        w[105] = 278_528;
        Arc::new(w)
    };

    // Without a font definition or widths: the run still fires with no
    // advance (upstream semantics preserved).
    let no_def = build(false);
    let rec = XdvParser::process(std::io::Cursor::new(&no_def), Recorder::default()).unwrap();
    let runs: Vec<&Event> = rec
        .0
        .events
        .iter()
        .filter(|e| matches!(e, Event::CharRun(..)))
        .collect();
    assert_eq!(runs.len(), 1);
    if let Event::CharRun(_, chars, xs, ys) = runs[0] {
        assert_eq!(*chars, vec![72, 105]);
        assert_eq!(*xs, vec![0, 0], "no widths injected: no advance");
        assert_eq!(*ys, vec![0, 0]);
    }
    assert!(
        rec.0.events.iter().all(|e| !matches!(e, Event::DefineFont(..))),
        "no fnt_def, no DefineFont event"
    );

    // With the definition + injected widths: positions advance per char and
    // the set_rule lands at h = 0 + w('H') + w('i').
    let with_def = build(true);
    let widths = Rc::new(RefCell::new(HashMap::new()));
    widths.borrow_mut().insert(5, table);

    let mut parser = XdvParser::new(Recorder::default());
    parser.set_char_widths(Rc::clone(&widths));
    parser.parse(&with_def).unwrap();
    let rec2 = parser.finish().unwrap();

    let def = rec2
        .events
        .iter()
        .find(|e| matches!(e, Event::DefineFont(..)))
        .expect("fnt_def must be forwarded");
    if let Event::DefineFont(area, name, num, checksum, scale, design) = def {
        assert_eq!((area.as_str(), name.as_str()), ("", "pplr8t"));
        assert_eq!(*num, 5);
        assert_eq!(*checksum, 0xDEAD_BEEF);
        assert_eq!(*scale, 655_360);
        assert_eq!(*design, 655_360);
    }

    let runs: Vec<&Event> = rec2
        .events
        .iter()
        .filter(|e| matches!(e, Event::CharRun(..)))
        .collect();
    assert_eq!(runs.len(), 1);
    if let Event::CharRun(_, chars, xs, ys) = runs[0] {
        assert_eq!(*chars, vec![72, 105]);
        assert_eq!(*xs, vec![0, 409_600], "h advances by the injected width");
        assert_eq!(*ys, vec![0, 0]);
    }
    if let Some(Event::Rule(x, ..)) = rec2.events.iter().find(|e| matches!(e, Event::Rule(..))) {
        assert_eq!(*x, 409_600 + 278_528, "rule after legacy text must not drift");
    } else {
        panic!("set_rule event missing");
    }
}
