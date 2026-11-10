package xdvjni;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.Locale;

/**
 * JNI 桥接层：声明与 Rust 侧 {@code native/src/lib.rs} 一一对应的 native 方法，
 * 并负责在类初始化时加载 {@code libxdvjni}。
 *
 * <p>本类是包内私有的实现细节，外部代码请使用 {@link Xdv} / {@link XdvParser}。
 *
 * <h2>本地库加载策略</h2>
 *
 * <ol>
 *   <li>先尝试 {@code System.loadLibrary("xdvjni")}，即从 {@code java.library.path}
 *       或系统库路径加载；（适用于把 {@code libxdvjni.so} 装到系统目录的场景）</li>
 *   <li>失败则从 classpath 资源 {@code xdvjni/native/<平台库文件名>} 解压到临时
 *       文件再 {@code System.load}；（Maven 打包时 native 库被塞进 jar 的
 *       {@code xdvjni/native/} 目录，因此 jar 是自包含的）</li>
 *   <li>两者都失败时抛出 {@link UnsatisfiedLinkError}，消息中会说明尝试过的方式。</li>
 * </ol>
 */
final class Native {

    static {
        load();
    }

    private Native() {
        // 工具类，不允许实例化。
    }

    private static void load() {
        // 1) 标准方式：java.library.path / 系统库路径。
        try {
            System.loadLibrary("xdvjni");
            return;
        } catch (UnsatisfiedLinkError ignored) {
            // 继续尝试 jar 内嵌副本。
        }

        // 2) jar 内嵌副本：xdvjni/native/<libxdvjni.so|libxdvjni.dylib|xdvjni.dll>
        String libName = libraryFileName();
        String resource = "native/" + libName;
        try (InputStream in = Native.class.getResourceAsStream(resource)) {
            if (in == null) {
                throw new UnsatisfiedLinkError(
                        "无法加载 xdvjni 本地库：java.library.path 中找不到 libxdvjni，"
                                + "classpath 中也不存在资源 " + resource);
            }
            String suffix = libName.substring(libName.lastIndexOf('.'));
            Path tmp = Files.createTempFile("xdvjni-", suffix);
            tmp.toFile().deleteOnExit();
            Files.copy(in, tmp, StandardCopyOption.REPLACE_EXISTING);
            System.load(tmp.toAbsolutePath().toString());
        } catch (IOException e) {
            UnsatisfiedLinkError error =
                    new UnsatisfiedLinkError("无法从 classpath 解压 xdvjni 本地库: " + e);
            error.initCause(e);
            throw error;
        }
    }

    /** 当前平台对应的本地库文件名。 */
    private static String libraryFileName() {
        String os = System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
        if (os.contains("win")) {
            return "xdvjni.dll";
        }
        if (os.contains("mac") || os.contains("darwin")) {
            return "libxdvjni.dylib";
        }
        return "libxdvjni.so";
    }

    // ---- 与 native/src/lib.rs 中 #[jni::jni_mangle("xdvjni.Native", ...)] 一一对应 ----

    /** 一次性解析完整缓冲区，返回消耗的字节数。 */
    static native long nativeParse(byte[] data, int off, int len, XdvEvents events);

    /** 创建流式解析器句柄（失败时抛 {@link XdvException}）。 */
    static native long nativeCreateParser(XdvEvents events);

    /** 向流式解析器喂一块数据，返回本块中被消耗的字节数。 */
    static native int nativeParserFeed(long handle, byte[] data, int off, int len);

    /** 断言流正常结束；数据被截断时抛 {@link XdvException}。 */
    static native void nativeParserFinish(long handle);

    /** 当前解析到的字节偏移。 */
    static native long nativeParserCurrentOffset(long handle);

    /** 解析器是否已结束。 */
    static native boolean nativeParserIsFinished(long handle);

    /** 释放解析器句柄（必须且只能调用一次）。 */
    static native void nativeParserDestroy(long handle);
}
