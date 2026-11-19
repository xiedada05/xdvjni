//! JNI bindings for the XDV/SPX parser (vendored in [`xdv`] from
//! `tectonic_xdv` 0.3.0, with legacy font/char-run patches).
//!
//! The native library `libxdvjni` exposes the static methods declared by the
//! Java class `xdvjni.Native`:
//!
//! * [`native_parse`] — one-shot parse of a complete byte array.
//! * `native_create_parser` / `native_parser_feed` / `native_parser_finish` /
//!   `native_parser_current_offset` / `native_parser_is_finished` /
//!   `native_parser_destroy` — incremental streaming parser driven by a
//!   handle.
//!
//! Errors are surfaced to Java as follows:
//!
//! * parser and argument errors throw `xdvjni.XdvException`;
//! * exceptions thrown by Java `XdvEvents` callbacks propagate unchanged;
//! * internal JNI failures and Rust panics throw `java.lang.RuntimeException`.
//!
//! Threading: each parser handle must be used by at most one thread at a
//! time. Events are delivered on the thread that feeds the parser.

mod cache;
mod events;
pub mod sample;
mod xdv;

// Re-export the vendored parser API under the crate root so downstream users
// (and integration tests) get the same surface that `tectonic_xdv` offered.
pub use xdv::XdvParser;
pub use xdv::{FileType, XdvError, XdvEvents};

use events::{BridgeError, JavaEvents};
use jni::EnvUnowned;
use jni::errors::ErrorPolicy;
use jni::objects::{JByteArray, JClass, JLongArray, JObject};
use jni::strings::JNIString;
use jni::sys::{jint, jlong};
use jni::vm::JavaVM;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

/// Shared per-font character width tables for legacy TFM fonts: font number →
/// slot-indexed advance widths in DVI units. Shared between the parser (read
/// during `set_char`) and the JNI `setCharWidths` entry point (written from
/// Java, typically from inside a `handleDefineFont` callback — DVI order
/// guarantees definitions precede their first use, and the parser never holds
/// a borrow of this cell while dispatching events).
type CharWidths = Rc<RefCell<HashMap<i32, Arc<Vec<i64>>>>>;

/// Native state behind a Java `xdvjni.XdvParser` handle.
struct ParserHandle {
    parser: Option<XdvParser<JavaEvents>>,
    finished: bool,
    /// Byte offset reached when `finish()` consumed the parser. Kept so that
    /// `currentOffset()` remains meaningful after a successful finish.
    final_offset: u64,
    /// Width tables shared with the parser (same `Rc`, same underlying cell).
    char_widths: CharWidths,
}

/// Error policy for native method boundaries: throws `xdvjni.XdvException`
/// for bridge errors, `java.lang.RuntimeException` for panics, and leaves
/// pending Java exceptions (from user callbacks) untouched.
struct ThrowXdvExAndDefault;

impl<T: Default, E: std::fmt::Display> ErrorPolicy<T, E> for ThrowXdvExAndDefault {
    type Captures<'unowned_env_local: 'native_method, 'native_method> = ();

    fn on_error<'unowned_env_local: 'native_method, 'native_method>(
        env: &mut jni::Env<'unowned_env_local>,
        _cap: &mut Self::Captures<'unowned_env_local, 'native_method>,
        err: E,
    ) -> jni::errors::Result<T> {
        if env.exception_check() {
            // A Java callback already threw; let that exception propagate.
            return Ok(T::default());
        }
        match cache::cache(env) {
            Ok(c) => {
                let _ = env.throw_new(&c.exception_class, JNIString::new(err.to_string()));
            }
            Err(_) => {
                let _ = env.throw(JNIString::new(format!("xdvjni: {err}")));
            }
        }
        Ok(T::default())
    }

    fn on_panic<'unowned_env_local: 'native_method, 'native_method>(
        env: &mut jni::Env<'unowned_env_local>,
        _cap: &mut Self::Captures<'unowned_env_local, 'native_method>,
        payload: Box<dyn std::any::Any + Send + 'static>,
    ) -> jni::errors::Result<T> {
        let msg = payload
            .downcast::<&'static str>()
            .map(|s| (*s).to_string())
            .unwrap_or_else(|p| {
                let _ = p; // avoid dropping the payload twice on panic paths
                "non-string panic payload".to_string()
            });
        let _ = env.throw(JNIString::new(format!("native xdvjni panic: {msg}")));
        Ok(T::default())
    }
}

fn copy_array(
    env: &mut jni::Env<'_>,
    data: &JByteArray<'_>,
    off: i32,
    len: i32,
) -> Result<Vec<u8>, BridgeError> {
    if off < 0 || len < 0 {
        return Err(BridgeError::BadArgs("off and len must be non-negative"));
    }
    // jbyte is i8; convert to u8 after copying.
    let mut raw = vec![0 as jni::sys::jbyte; len as usize];
    if len > 0 {
        data.get_region(env, off, &mut raw).map_err(BridgeError::Jni)?;
    }
    Ok(raw.into_iter().map(|b| b as u8).collect())
}

fn parser_handle(handle: jlong) -> Result<*mut ParserHandle, BridgeError> {
    let ptr = handle as *mut ParserHandle;
    if ptr.is_null() {
        Err(BridgeError::BadState("null parser handle"))
    } else {
        Ok(ptr)
    }
}

/// `static native long nativeParse(byte[] data, int off, int len, XdvEvents events)`
/// — parse a complete XDV/SPX buffer, returning the number of bytes consumed.
#[jni::jni_mangle("xdvjni.Native", "nativeParse")]
pub fn native_parse<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    data: JByteArray<'local>,
    off: jint,
    len: jint,
    events: JObject<'local>,
) -> jlong {
    env.with_env(|env| -> Result<jlong, BridgeError> {
        cache::cache(env).map_err(BridgeError::Jni)?;
        let buf = copy_array(env, &data, off, len)?;
        let vm: JavaVM = env.get_java_vm().map_err(BridgeError::Jni)?;
        let events_ref = env.new_global_ref(&events).map_err(BridgeError::Jni)?;
        let handler = JavaEvents::new(vm, events_ref);
        let (_handler, n) = XdvParser::process(std::io::Cursor::new(buf), handler)?;
        Ok(n as jlong)
    })
    .resolve::<ThrowXdvExAndDefault>()
}

/// `static native long nativeCreateParser(XdvEvents events)` — create a
/// streaming parser handle.
#[jni::jni_mangle("xdvjni.Native", "nativeCreateParser")]
pub fn native_create_parser<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    events: JObject<'local>,
) -> jlong {
    env.with_env(|env| -> Result<jlong, BridgeError> {
        cache::cache(env).map_err(BridgeError::Jni)?;
        let vm: JavaVM = env.get_java_vm().map_err(BridgeError::Jni)?;
        let events_ref = env.new_global_ref(&events).map_err(BridgeError::Jni)?;
        let parser = XdvParser::new(JavaEvents::new(vm, events_ref));
        let char_widths: CharWidths = Rc::new(RefCell::new(HashMap::new()));
        let mut parser = parser;
        parser.set_char_widths(Rc::clone(&char_widths));
        let handle = Box::new(ParserHandle {
            parser: Some(parser),
            finished: false,
            final_offset: 0,
            char_widths,
        });
        Ok(Box::into_raw(handle) as jlong)
    })
    .resolve::<ThrowXdvExAndDefault>()
}

/// `static native int nativeParserFeed(long handle, byte[] data, int off, int len)`
/// — feed a chunk, returning the number of bytes consumed.
#[jni::jni_mangle("xdvjni.Native", "nativeParserFeed")]
pub fn native_parser_feed<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
    data: JByteArray<'local>,
    off: jint,
    len: jint,
) -> jint {
    env.with_env(|env| -> Result<jint, BridgeError> {
        // Safety: handles originate from `native_create_parser` and the Java
        // API contract guarantees exclusive, single-threaded use.
        let cell = unsafe { &mut *parser_handle(handle)? };
        let parser = cell
            .parser
            .as_mut()
            .ok_or(BridgeError::BadState("parser already finished"))?;
        let buf = copy_array(env, &data, off, len)?;
        let (consumed, cont) = parser.parse(&buf)?;
        if !cont {
            cell.finished = true;
        }
        Ok(consumed as jint)
    })
    .resolve::<ThrowXdvExAndDefault>()
}

/// `static native void nativeParserSetCharWidths(long handle, int fontNum, long[] widths)`
/// — install a slot-indexed character advance width table (DVI units) for one
/// font number, used by legacy `set_char` handling. `widths[slot]` is the
/// advance of slot `slot`; out-of-range slots advance by zero. Passing a
/// `null` array removes the font's table. Safe to call from inside an event
/// callback: the parser never holds a borrow of the width cell while
/// dispatching events.
#[jni::jni_mangle("xdvjni.Native", "nativeParserSetCharWidths")]
pub fn native_parser_set_char_widths<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
    font_num: jint,
    widths: JLongArray<'local>,
) {
    env.with_env(|env| -> Result<(), BridgeError> {
        // Safety: see `native_parser_feed`.
        let cell = unsafe { &*parser_handle(handle)? };
        let table: Option<Arc<Vec<i64>>> = if widths.is_null() {
            None
        } else {
            let n = widths.len(env).map_err(BridgeError::Jni)?;
            let mut buf = vec![0 as jni::sys::jlong; n];
            if n > 0 {
                widths
                    .get_region(env, 0, &mut buf)
                    .map_err(BridgeError::Jni)?;
            }
            Some(Arc::new(buf))
        };
        let mut map = cell.char_widths.borrow_mut();
        match table {
            Some(t) => {
                map.insert(font_num, t);
            }
            None => {
                map.remove(&font_num);
            }
        }
        Ok(())
    })
    .resolve::<ThrowXdvExAndDefault>()
}

/// `static native void nativeParserFinish(long handle)` — assert the stream
/// ended cleanly (throws `XdvException` when truncated).
#[jni::jni_mangle("xdvjni.Native", "nativeParserFinish")]
pub fn native_parser_finish<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
) {
    env.with_env(|_env| -> Result<(), BridgeError> {
        // Safety: see `native_parser_feed`.
        let cell = unsafe { &mut *parser_handle(handle)? };
        let parser = cell
            .parser
            .take()
            .ok_or(BridgeError::BadState("parser already finished"))?;
        // Capture the final offset before `finish()` consumes the parser.
        let offset = parser.current_offset();
        parser.finish()?;
        cell.final_offset = offset;
        cell.finished = true;
        Ok(())
    })
    .resolve::<ThrowXdvExAndDefault>()
}

/// `static native long nativeParserCurrentOffset(long handle)`.
#[jni::jni_mangle("xdvjni.Native", "nativeParserCurrentOffset")]
pub fn native_parser_current_offset<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
) -> jlong {
    env.with_env(|_env| -> Result<jlong, BridgeError> {
        // Safety: see `native_parser_feed`.
        let cell = unsafe { &*parser_handle(handle)? };
        let offset = match cell.parser.as_ref() {
            Some(parser) => parser.current_offset(),
            // After `finish()` the parser is consumed; report the offset it
            // reached instead of failing.
            None => cell.final_offset,
        };
        Ok(offset as jlong)
    })
    .resolve::<ThrowXdvExAndDefault>()
}

/// `static native boolean nativeParserIsFinished(long handle)`.
#[jni::jni_mangle("xdvjni.Native", "nativeParserIsFinished")]
pub fn native_parser_is_finished<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
) -> jni::sys::jboolean {
    env.with_env(|_env| -> Result<jni::sys::jboolean, BridgeError> {
        // Safety: see `native_parser_feed`.
        let cell = unsafe { &*parser_handle(handle)? };
        Ok(cell.finished as jni::sys::jboolean)
    })
    .resolve::<ThrowXdvExAndDefault>()
}

/// `static native void nativeParserDestroy(long handle)` — free the parser.
#[jni::jni_mangle("xdvjni.Native", "nativeParserDestroy")]
pub fn native_parser_destroy<'local>(
    _env: EnvUnowned<'local>,
    _class: JClass<'local>,
    handle: jlong,
) {
    if let Ok(ptr) = parser_handle(handle) {
        // Safety: ownership transfers back from the Java handle; the Java
        // API guarantees this is called exactly once per handle.
        drop(unsafe { Box::from_raw(ptr) });
    }
}
