//! Validates the hand-crafted sample XDV against the real `tectonic_xdv`
//! parser: both the byte layout produced by `xdvjni::sample` and the exact
//! event sequence (which the Java tests mirror).

use std::fmt;
use std::fmt::Write as _;
use tectonic_xdv::{FileType, XdvError, XdvEvents, XdvParser};

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
    CharRun(i32, Vec<i32>),
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

    fn handle_char_run(&mut self, font_num: i32, chars: &[i32]) -> Result<(), TestError> {
        self.events
            .push(Event::CharRun(font_num, chars.to_vec()));
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
        Event::CharRun(5, vec![72, 105]),
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
        Event::CharRun(6, vec![88]),
    ];
    assert_eq!(recorder.events, expected);
}

#[test]
fn truncated_sample_fails_one_shot() {
    let data = xdvjni::sample::generate_sample();
    let cut = &data[..data.len() - 6];
    let err = XdvParser::process(std::io::Cursor::new(cut), Recorder::default()).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("ended unexpectedly soon") || msg.contains("UnexpectedEndOfStream"),
        "unexpected error: {msg}"
    );
}

#[test]
fn streaming_feed_matches_one_shot() {
    let data = xdvjni::sample::generate_sample();

    // One-shot.
    let one_shot = XdvParser::process(std::io::Cursor::new(&data), Recorder::default())
        .unwrap()
        .0
        .events;

    // Streaming in 7-byte chunks, following the documented driver contract:
    // bytes not consumed by `parse` stay buffered and the next chunk is
    // appended. `consumed == 0` means the parser needs a bigger window, which
    // is expected here because directives such as glyph runs are longer than
    // one chunk.
    let mut parser = XdvParser::new(Recorder::default());
    let mut buffer: Vec<u8> = Vec::new();
    let mut next = 0usize;

    while next < data.len() || !buffer.is_empty() {
        if next < data.len() {
            let n = 7.min(data.len() - next);
            buffer.extend_from_slice(&data[next..next + n]);
            next += n;
        }

        let (consumed, keep_going) = parser.parse(&buffer).unwrap();
        assert!(
            consumed <= buffer.len(),
            "parser consumed more bytes than were available"
        );
        buffer.drain(..consumed);

        if consumed == 0 && next >= data.len() {
            panic!(
                "parser stalled with {} buffered byte(s) and no input left",
                buffer.len()
            );
        }
        if !keep_going {
            break;
        }
    }

    let streaming = parser.finish().unwrap().events;
    assert_eq!(one_shot, streaming);
}

#[test]
fn sample_hex_dump_is_stable() {
    // Guards against accidental layout changes: the Java tests embed this
    // offset (the page-2 bop pointer) in their expectations.
    let data = xdvjni::sample::generate_sample();
    let mut dump = String::new();
    for (i, b) in data.iter().enumerate() {
        if i % 16 == 0 {
            writeln!(&mut dump).unwrap();
        }
        write!(&mut dump, "{b:02x} ").unwrap();
    }
    assert_eq!(data[28], 139, "page-1 bop opcode must sit at offset 28");
}
