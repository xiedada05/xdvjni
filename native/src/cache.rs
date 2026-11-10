//! Lazily initialized cache of JNI classes, method IDs and global references.
//!
//! Java reflection lookups (`FindClass`/`GetMethodID`) are far too expensive
//! to repeat per event, so everything the bridge needs is resolved once and
//! kept alive for the process lifetime.

use std::sync::OnceLock;

use jni::objects::{JClass, JMethodID, JObject};
use jni::refs::Global;
use jni::{Env, jni_sig, jni_str};

/// Cached JNI metadata used by the event bridge.
pub struct JniCache {
    /// `xdvjni.XdvEvents` interface class. The global reference is kept for
    /// the lifetime of the process so the class cannot be unloaded, which
    /// would invalidate the cached method IDs.
    #[allow(dead_code)]
    pub events_class: Global<JClass<'static>>,
    pub handle_header: JMethodID,
    pub handle_begin_page: JMethodID,
    pub handle_special: JMethodID,
    pub handle_char_run: JMethodID,
    pub handle_glyph_run: JMethodID,
    pub handle_text_and_glyphs: JMethodID,
    pub handle_define_native_font: JMethodID,
    pub handle_rule: JMethodID,

    /// The `FileType.XDV` / `FileType.SPX` enum constants.
    pub filetype_xdv: Global<JObject<'static>>,
    pub filetype_spx: Global<JObject<'static>>,

    /// `java.lang.Integer` (for boxing the optional font parameters).
    pub integer_class: Global<JClass<'static>>,

    /// `xdvjni.XdvException`, raised for parser errors.
    pub exception_class: Global<JClass<'static>>,
}

static CACHE: OnceLock<JniCache> = OnceLock::new();

/// Get the process-wide cache, initializing it on first use.
pub fn cache(env: &mut Env<'_>) -> jni::errors::Result<&'static JniCache> {
    if let Some(c) = CACHE.get() {
        return Ok(c);
    }
    let built = build(env)?;
    Ok(CACHE.get_or_init(|| built))
}

fn build(env: &mut Env<'_>) -> jni::errors::Result<JniCache> {
    let events_class_local = env.find_class(jni_str!("xdvjni/XdvEvents"))?;
    let events_class = env.new_global_ref(events_class_local)?;

    let handle_header = env.get_method_id(
        &events_class,
        jni_str!("handleHeader"),
        jni_sig!((filetype: xdvjni.FileType, comment: [jbyte])),
    )?;
    let handle_begin_page = env.get_method_id(
        &events_class,
        jni_str!("handleBeginPage"),
        jni_sig!((counters: [jint], previous_bop: jint)),
    )?;
    let handle_special = env.get_method_id(
        &events_class,
        jni_str!("handleSpecial"),
        jni_sig!((x: jint, y: jint, contents: [jbyte])),
    )?;
    let handle_char_run = env.get_method_id(
        &events_class,
        jni_str!("handleCharRun"),
        jni_sig!((font_num: jint, chars: [jint])),
    )?;
    let handle_glyph_run = env.get_method_id(
        &events_class,
        jni_str!("handleGlyphRun"),
        jni_sig!((font_num: jint, glyphs: [jchar], x: [jint], y: [jint])),
    )?;
    let handle_text_and_glyphs = env.get_method_id(
        &events_class,
        jni_str!("handleTextAndGlyphs"),
        jni_sig!((
            font_num: jint,
            text: java.lang.String,
            width: jint,
            glyphs: [jchar],
            x: [jint],
            y: [jint],
        )),
    )?;
    let handle_define_native_font = env.get_method_id(
        &events_class,
        jni_str!("handleDefineNativeFont"),
        jni_sig!((
            name: java.lang.String,
            font_num: jint,
            size: jint,
            face_index: jint,
            color_rgba: java.lang.Integer,
            extend: java.lang.Integer,
            slant: java.lang.Integer,
            embolden: java.lang.Integer,
        )),
    )?;
    let handle_rule = env.get_method_id(
        &events_class,
        jni_str!("handleRule"),
        jni_sig!((x: jint, y: jint, height: jint, width: jint)),
    )?;

    let filetype_class = env.find_class(jni_str!("xdvjni/FileType"))?;
    let xdv = env.get_static_field(&filetype_class, jni_str!("XDV"), jni_sig!(xdvjni.FileType))?;
    let spx = env.get_static_field(&filetype_class, jni_str!("SPX"), jni_sig!(xdvjni.FileType))?;
    let filetype_xdv = env.new_global_ref(xdv.l()?)?;
    let filetype_spx = env.new_global_ref(spx.l()?)?;

    let integer_class_local = env.find_class(jni_str!("java/lang/Integer"))?;
    let integer_class = env.new_global_ref(integer_class_local)?;

    let exception_class_local = env.find_class(jni_str!("xdvjni/XdvException"))?;
    let exception_class = env.new_global_ref(exception_class_local)?;

    Ok(JniCache {
        events_class,
        handle_header,
        handle_begin_page,
        handle_special,
        handle_char_run,
        handle_glyph_run,
        handle_text_and_glyphs,
        handle_define_native_font,
        handle_rule,
        filetype_xdv,
        filetype_spx,
        integer_class,
        exception_class,
    })
}
