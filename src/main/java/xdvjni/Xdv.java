package xdvjni;

import java.util.Objects;

/**
 * 一次性 XDV/SPX 解析入口，对应 Rust {@code tectonic_xdv::XdvParser::process}。
 *
 * <p>适合解析整块内存中的完整文件。需要边读边解析（例如从流里读大文件）时请用
 * {@link XdvParser}。
 *
 * <pre>{@code
 * byte[] data = Files.readAllBytes(Path.of("doc.xdv"));
 * Xdv.parse(data, new XdvEvents() {
 *     @Override
 *     public void handleHeader(FileType filetype, byte[] comment) {
 *         System.out.println("类型: " + filetype);
 *     }
 * });
 * }</pre>
 */
public final class Xdv {

    private Xdv() {
        // 工具类，不允许实例化。
    }

    /**
     * 解析整个字节数组。
     *
     * @param data   完整的 XDV/SPX 数据
     * @param events 事件回调
     * @return 解析器实际消耗的字节数（正常情况下等于 {@code data.length}）
     * @throws XdvException 数据格式错误、被截断或包含非法 opcode
     * @throws RuntimeException {@code events} 回调自行抛出的异常会原样传播
     * @throws NullPointerException 参数为 {@code null}
     */
    public static long parse(byte[] data, XdvEvents events) {
        Objects.requireNonNull(data, "data");
        return parse(data, 0, data.length, events);
    }

    /**
     * 解析字节数组的一段区间。
     *
     * @param data   数据缓冲区
     * @param off    起始偏移
     * @param len    长度
     * @param events 事件回调
     * @return 解析器实际消耗的字节数（正常情况下等于 {@code len}）
     * @throws XdvException 数据格式错误、被截断或包含非法 opcode
     * @throws RuntimeException {@code events} 回调自行抛出的异常会原样传播
     * @throws IndexOutOfBoundsException {@code off}/{@code len} 越界
     * @throws NullPointerException 参数为 {@code null}
     */
    public static long parse(byte[] data, int off, int len, XdvEvents events) {
        Objects.requireNonNull(data, "data");
        Objects.requireNonNull(events, "events");
        checkRange(data.length, off, len);
        return Native.nativeParse(data, off, len, events);
    }

    /**
     * 校验 {@code [off, off + len)} 落在 {@code [0, size)} 内。
     *
     * <p>native 层也会再校验一次（不合法时抛 {@link XdvException}），这里提前做是为了
     * 抛出符合 Java 习惯的异常类型。
     */
    static void checkRange(int size, int off, int len) {
        if (off < 0 || len < 0 || off > size - len) {
            throw new IndexOutOfBoundsException(
                    "off=" + off + ", len=" + len + ", size=" + size);
        }
    }
}
