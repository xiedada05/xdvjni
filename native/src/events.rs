//! [`XdvEvents`] implementation that forwards parser events to a Java
//! `xdvjni.XdvEvents` instance.

use std::fmt;

use jni::objects::{JObject, JPrimitiveArray};
use jni::refs::Global;
use jni::signature::{Primitive, ReturnType};
use jni::vm::{AttachConfig, AttachmentExceptionPolicy};
use jni::{Env, JValue, jni_sig, jni_str};
use tectonic_xdv::{FileType, XdvError, XdvEvents};

use crate::cache::{self, JniCache};

/// Error type for the Java event bridge.
///
/// The `From<io::Error>` impl satisfies the `XdvParser::process` bound; the
/// `From<jni::errors::Error>` impl satisfies the closure bounds of the
/// `jni` crate's attach helpers.
#[derive(Debug)]
pub enum BridgeError {
    /// Error raised by the underlying XDV/SPX parser.
    Xdv(XdvError),
    /// A Java exception is pending on the calling thread, typically thrown
    /// by an `XdvEvents` callback. The pending exception propagates to the
    /// Java caller unchanged.
    JavaPending,
    /// A JNI machinery failure.
    Jni(jni::errors::Error),
    /// An I/O failure while reading the input stream.
    Io(String),
    /// Invalid arguments passed from Java.
    BadArgs(&'static str),
    /// A handle was used after being finished or destroyed.
    BadState(&'static str),
}

impl From<XdvError> for BridgeError {
    fn from(e: XdvError) -> Self {
        BridgeError::Xdv(e)
    }
}

impl From<std::io::Error> for BridgeError {
    fn from(e: std::io::Error) -> Self {
        BridgeError::Io(e.to_string())
    }
}

impl From<jni::errors::Error> for BridgeError {
    fn from(e: jni::errors::Error) -> Self {
        BridgeError::Jni(e)
    }
}

impl fmt::Display for BridgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BridgeError::Xdv(e) => match e {
                XdvError::Malformed(offset) => {
                    write!(f, "malformed XDV/SPX input at byte offset {offset}")
                }
                XdvError::IllegalOpcode(op, offset) => {
                    write!(f, "illegal opcode {op} (0x{op:02x}) at byte offset {offset}")
                }
                XdvError::UnexpectedEndOfStream => {
                    write!(f, "unexpected end of XDV/SPX stream (truncated input)")
                }
                XdvError::FromUTF8(offset) => {
                    write!(f, "invalid UTF-8 in XDV/SPX input at byte offset {offset}")
                }
                XdvError::FromUTF16(offset) => {
                    write!(f, "invalid UTF-16 in XDV/SPX input at byte offset {offset}")
                }
            },
            BridgeError::JavaPending => write!(f, "Java exception raised by an XdvEvents callback"),
            BridgeError::Jni(e) => write!(f, "JNI error: {e}"),
            BridgeError::Io(m) => write!(f, "I/O error: {m}"),
            BridgeError::BadArgs(m) => write!(f, "invalid argument: {m}"),
            BridgeError::BadState(m) => write!(f, "invalid parser state: {m}"),
        }
    }
}

impl std::error::Error for BridgeError {}

/// Forwards XDV parser events to a global reference to a Java
/// `xdvjni.XdvEvents` implementation.
pub struct JavaEvents {
    vm: jni::vm::JavaVM,
    events: Global<JObject<'static>>,
}

impl JavaEvents {
    pub fn new(vm: jni::vm::JavaVM, events: Global<JObject<'static>>) -> Self {
        JavaEvents { vm, events }
    }

    /// Attach the current thread (cheap when already attached, which is the
    /// normal case) and run one event dispatch inside a fresh local frame.
    fn dispatch<F>(&self, f: F) -> Result<(), BridgeError>
    where
        F: FnOnce(&mut Env<'_>, &'static JniCache) -> jni::errors::Result<()>,
    {
        self.vm.attach_current_thread_with_config(
            || {
                AttachConfig::new()
                    // Leave pending exceptions completely untouched so that
                    // exceptions thrown by Java callbacks propagate as-is.
                    .exceptions_policy(AttachmentExceptionPolicy::Ignore)
            },
            Some(16),
            |env| -> Result<(), BridgeError> {
                let cache = cache::cache(env).map_err(BridgeError::Jni)?;
                f(env, cache)?;
                if env.exception_check() {
                    Err(BridgeError::JavaPending)
                } else {
                    Ok(())
                }
            },
        )
    }

    /// Call a `void` method with the given method ID and raw arguments.
    ///
    /// # Safety
    ///
    /// `method` must belong to the class of `self.events` and match the
    /// argument types encoded in `args`.
    fn call_void(
        &self,
        env: &mut Env<'_>,
        method: jni::objects::JMethodID,
        args: &[jni::sys::jvalue],
    ) -> jni::errors::Result<()> {
        // Safety: every call site pairs a cached method ID of the
        // `xdvjni.XdvEvents` interface with arguments of matching types.
        unsafe {
            env.call_method_unchecked(
                self.events.as_obj(),
                method,
                ReturnType::Primitive(Primitive::Void),
                args,
            )?;
        }
        Ok(())
    }
}

impl XdvEvents for JavaEvents {
    type Error = BridgeError;

    fn handle_header(&mut self, filetype: FileType, comment: &[u8]) -> Result<(), Self::Error> {
        self.dispatch(|env, cache| {
            let ft = match filetype {
                FileType::Xdv => &cache.filetype_xdv,
                FileType::Spx => &cache.filetype_spx,
            };
            let comment = env.byte_array_from_slice(comment)?;
            let comment: JObject = comment.into();
            self.call_void(
                env,
                cache.handle_header,
                &[
                    JValue::Object(ft.as_obj()).as_jni(),
                    JValue::Object(&comment).as_jni(),
                ],
            )
        })
    }

    fn handle_begin_page(&mut self, counters: &[i32], previous_bop: i32) -> Result<(), Self::Error> {
        self.dispatch(|env, cache| {
            let counters = int_array(env, counters)?;
            let counters: JObject = counters.into();
            self.call_void(
                env,
                cache.handle_begin_page,
                &[
                    JValue::Object(&counters).as_jni(),
                    JValue::Int(previous_bop).as_jni(),
                ],
            )
        })
    }

    fn handle_special(&mut self, x: i32, y: i32, contents: &[u8]) -> Result<(), Self::Error> {
        self.dispatch(|env, cache| {
            let contents = env.byte_array_from_slice(contents)?;
            let contents: JObject = contents.into();
            self.call_void(
                env,
                cache.handle_special,
                &[
                    JValue::Int(x).as_jni(),
                    JValue::Int(y).as_jni(),
                    JValue::Object(&contents).as_jni(),
                ],
            )
        })
    }

    fn handle_char_run(&mut self, font_num: i32, chars: &[i32]) -> Result<(), Self::Error> {
        self.dispatch(|env, cache| {
            let chars = int_array(env, chars)?;
            let chars: JObject = chars.into();
            self.call_void(
                env,
                cache.handle_char_run,
                &[
                    JValue::Int(font_num).as_jni(),
                    JValue::Object(&chars).as_jni(),
                ],
            )
        })
    }

    fn handle_glyph_run(
        &mut self,
        font_num: i32,
        glyphs: &[u16],
        x: &[i32],
        y: &[i32],
    ) -> Result<(), Self::Error> {
        self.dispatch(|env, cache| {
            let glyphs = char_array(env, glyphs)?;
            let x = int_array(env, x)?;
            let y = int_array(env, y)?;
            let glyphs: JObject = glyphs.into();
            let x: JObject = x.into();
            let y: JObject = y.into();
            self.call_void(
                env,
                cache.handle_glyph_run,
                &[
                    JValue::Int(font_num).as_jni(),
                    JValue::Object(&glyphs).as_jni(),
                    JValue::Object(&x).as_jni(),
                    JValue::Object(&y).as_jni(),
                ],
            )
        })
    }

    fn handle_text_and_glyphs(
        &mut self,
        font_num: i32,
        text: &str,
        width: i32,
        glyphs: &[u16],
        x: &[i32],
        y: &[i32],
    ) -> Result<(), Self::Error> {
        self.dispatch(|env, cache| {
            let text = env.new_string(text)?;
            let glyphs = char_array(env, glyphs)?;
            let x = int_array(env, x)?;
            let y = int_array(env, y)?;
            let text: JObject = text.into();
            let glyphs: JObject = glyphs.into();
            let x: JObject = x.into();
            let y: JObject = y.into();
            self.call_void(
                env,
                cache.handle_text_and_glyphs,
                &[
                    JValue::Int(font_num).as_jni(),
                    JValue::Object(&text).as_jni(),
                    JValue::Int(width).as_jni(),
                    JValue::Object(&glyphs).as_jni(),
                    JValue::Object(&x).as_jni(),
                    JValue::Object(&y).as_jni(),
                ],
            )
        })
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
    ) -> Result<(), Self::Error> {
        self.dispatch(|env, cache| {
            let name = env.new_string(name)?;
            let name: JObject = name.into();
            let color = boxed_int(env, cache, color_rgba)?;
            let extend = boxed_int(env, cache, extend)?;
            let slant = boxed_int(env, cache, slant)?;
            let embolden = boxed_int(env, cache, embolden)?;
            let color = color.unwrap_or(JObject::null());
            let extend = extend.unwrap_or(JObject::null());
            let slant = slant.unwrap_or(JObject::null());
            let embolden = embolden.unwrap_or(JObject::null());
            self.call_void(
                env,
                cache.handle_define_native_font,
                &[
                    JValue::Object(&name).as_jni(),
                    JValue::Int(font_num).as_jni(),
                    JValue::Int(size).as_jni(),
                    JValue::Int(face_index as i32).as_jni(),
                    JValue::Object(&color).as_jni(),
                    JValue::Object(&extend).as_jni(),
                    JValue::Object(&slant).as_jni(),
                    JValue::Object(&embolden).as_jni(),
                ],
            )
        })
    }

    fn handle_rule(&mut self, x: i32, y: i32, height: i32, width: i32) -> Result<(), Self::Error> {
        self.dispatch(|env, cache| {
            self.call_void(
                env,
                cache.handle_rule,
                &[
                    JValue::Int(x).as_jni(),
                    JValue::Int(y).as_jni(),
                    JValue::Int(height).as_jni(),
                    JValue::Int(width).as_jni(),
                ],
            )
        })
    }
}

fn int_array<'e>(
    env: &mut Env<'e>,
    vals: &[i32],
) -> jni::errors::Result<JPrimitiveArray<'e, jni::sys::jint>> {
    let arr = JPrimitiveArray::<jni::sys::jint>::new(env, vals.len())?;
    if !vals.is_empty() {
        arr.set_region(env, 0, vals)?;
    }
    Ok(arr)
}

fn char_array<'e>(
    env: &mut Env<'e>,
    vals: &[u16],
) -> jni::errors::Result<JPrimitiveArray<'e, jni::sys::jchar>> {
    let arr = JPrimitiveArray::<jni::sys::jchar>::new(env, vals.len())?;
    if !vals.is_empty() {
        arr.set_region(env, 0, vals)?;
    }
    Ok(arr)
}

fn boxed_int<'e>(
    env: &mut Env<'e>,
    cache: &JniCache,
    v: Option<u32>,
) -> jni::errors::Result<Option<JObject<'e>>> {
    match v {
        None => Ok(None),
        Some(v) => {
            let boxed = env.call_static_method(
                &cache.integer_class,
                jni_str!("valueOf"),
                jni_sig!((value: jint) -> java.lang.Integer),
                &[JValue::Int(v as i32)],
            )?;
            Ok(Some(boxed.l()?))
        }
    }
}
