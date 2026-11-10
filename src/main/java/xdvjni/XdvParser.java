package xdvjni;

import java.lang.ref.Cleaner;
import java.util.Objects;

/**
 * 流式 XDV/SPX 解析器，对应 Rust {@code tectonic_xdv::XdvParser}。
 *
 * <p>用于从网络流、文件流等数据源边读边解析：反复调用 {@link #feed} 喂入数据，
 * 数据结束时调用 {@link #finish()} 确认流完整。解析器持有 native 句柄，必须
 * {@link #close()} 释放；推荐用 try-with-resources：
 *
 * <pre>{@code
 * try (XdvParser parser = new XdvParser(events)) {
 *     for (byte[] chunk : chunks) {
 *         int consumed = parser.feed(chunk);
 *         // consumed < chunk.length 表示剩余字节还不足以构成下一条指令，
 *         // 需要把它们保留下来，与后续数据拼接后重新喂入。
 *     }
 *     parser.finish();
 * }
 * }</pre>
 *
 * <h2>线程模型</h2>
 *
 * <p>本类<b>非线程安全</b>：同一个解析器同一时刻只能被一个线程使用。事件回调在
 * 调用 {@link #feed} / {@link #finish()} 的线程上执行。
 *
 * <h2>句柄生命周期</h2>
 *
 * <p>{@link #close()} 是幂等的；调用后所有方法都会抛
 * {@link IllegalStateException}。若调用方忘记 close，{@link Cleaner} 会在实例被
 * 回收时兜底释放 native 句柄。
 */
public final class XdvParser implements AutoCloseable {

    private static final Cleaner CLEANER = Cleaner.create();

    /** 持有 native 指针与释放动作。不能持有 {@link XdvParser} 自身的强引用。 */
    private static final class Handle implements Runnable {
        private long pointer;

        Handle(long pointer) {
            this.pointer = pointer;
        }

        long pointer() {
            return pointer;
        }

        @Override
        public void run() {
            long p = pointer;
            pointer = 0L;
            if (p != 0L) {
                Native.nativeParserDestroy(p);
            }
        }
    }

    private final Handle handle;
    private final Cleaner.Cleanable cleanable;

    /**
     * 创建流式解析器。
     *
     * @param events 事件回调，回调期间持有其强引用
     * @throws NullPointerException {@code events} 为 {@code null}
     * @throws XdvException         native 层初始化失败
     */
    public XdvParser(XdvEvents events) {
        Objects.requireNonNull(events, "events");
        this.handle = new Handle(Native.nativeCreateParser(events));
        this.cleanable = CLEANER.register(this, this.handle);
    }

    /**
     * 喂入一整块数据，返回其中被消耗的字节数。
     *
     * <p>返回值可能小于 {@code data.length}：解析器只处理完整指令，尾部不完整的
     * 字节不会被消耗，调用方应把它们保留下来，与后续数据拼接后再次喂入。
     * 返回 0 表示现有数据还不足以解析下一条指令，需要积累更多输入。
     *
     * @param data 数据块
     * @return 本次调用消耗的字节数
     * @throws XdvException         数据格式错误或包含非法 opcode
     * @throws RuntimeException     回调自行抛出的异常原样传播
     * @throws IllegalStateException 解析器已关闭或已结束
     */
    public int feed(byte[] data) {
        Objects.requireNonNull(data, "data");
        return feed(data, 0, data.length);
    }

    /**
     * 喂入数据块的一段区间。
     *
     * @param data 数据块
     * @param off  起始偏移
     * @param len  长度
     * @return 本次调用消耗的字节数
     * @throws XdvException          数据格式错误或包含非法 opcode
     * @throws RuntimeException      回调自行抛出的异常原样传播
     * @throws IllegalStateException 解析器已关闭或已结束
     * @throws IndexOutOfBoundsException {@code off}/{@code len} 越界
     */
    public int feed(byte[] data, int off, int len) {
        Objects.requireNonNull(data, "data");
        Xdv.checkRange(data.length, off, len);
        return Native.nativeParserFeed(pointer(), data, off, len);
    }

    /**
     * 当前解析到的字节偏移。
     *
     * <p>在 {@link #finish()} 成功后依然可用，返回文件总字节数。
     *
     * @return 已处理字节数
     * @throws IllegalStateException 解析器已关闭
     */
    public long currentOffset() {
        return Native.nativeParserCurrentOffset(pointer());
    }

    /**
     * 解析器是否已结束。
     *
     * @return native 解析器是否已完成
     * @throws IllegalStateException 解析器已关闭
     */
    public boolean isFinished() {
        return Native.nativeParserIsFinished(pointer());
    }

    /**
     * 断言输入流已正常结束，并释放底层解析状态。
     *
     * <p>应在喂完全部数据后调用。若数据被截断（缺少 postamble / double
     * postamble），本方法抛 {@link XdvException}。
     *
     * @throws XdvException          数据被截断
     * @throws IllegalStateException 解析器已关闭或已结束
     */
    public void finish() {
        Native.nativeParserFinish(pointer());
    }

    /**
     * 释放 native 句柄。幂等；调用后本实例不可再使用。
     */
    @Override
    public void close() {
        cleanable.clean();
    }

    /** 取 native 指针，已关闭时抛 {@link IllegalStateException}。 */
    private long pointer() {
        long p = handle.pointer();
        if (p == 0L) {
            throw new IllegalStateException("XdvParser 已关闭");
        }
        return p;
    }
}
