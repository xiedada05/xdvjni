package xdvjni;

/**
 * XDV/SPX 解析事件的回调接口，与 Rust {@code tectonic_xdv::XdvEvents} trait 一一对应。
 *
 * <p>解析器在遇到相应结构时同步回调本接口。所有方法都提供了空实现，调用方只需
 * 覆盖自己关心的事件（与 Rust trait 的默认实现一致）。
 *
 * <h2>线程模型</h2>
 *
 * <p>回调在<b>调用 {@link Xdv#parse} 或 {@link XdvParser#feed} 的那个线程</b>上执行。
 * 因此回调里可以安全地访问该线程的普通局部状态，无需额外同步；但不要在回调里
 * 重入同一个解析器（无论一次性 {@link Xdv} 还是流式 {@link XdvParser}）。
 *
 * <h2>异常</h2>
 *
 * <p>回调中抛出的任何异常都会立即中止解析，并<b>原样</b>传播给
 * {@link Xdv#parse} / {@link XdvParser#feed} 的调用方（不会被包装成
 * {@link XdvException}）。{@link AutoCloseable#close()} 之类的清理逻辑应由调用方
 * 用 try-with-resources 保证。
 *
 * <h2>数据所有权</h2>
 *
 * <p>回调参数中的数组和字符串都是 native 层为本次事件新建的 Java 对象，回调返回后
 * 不再被 native 层引用，可以自由保存或修改。
 *
 * <h2>无符号字段</h2>
 *
 * <p>Rust 侧 {@code u32} 字段（{@code faceIndex} 以及 4 个可选的字体参数）以 Java
 * {@code int} 的位模式传递，可能为负。需要无符号值时请使用
 * {@link Integer#toUnsignedLong(int)} 或 {@link Integer#toUnsignedString(int)}
 * 读取。
 */
public interface XdvEvents {

    /**
     * 文件头（preamble）。
     *
     * @param filetype 文件类型（{@link FileType#XDV} 或 {@link FileType#SPX}）
     * @param comment  原始注释字节，是否 UTF-8 由调用方自行判断
     */
    default void handleHeader(FileType filetype, byte[] comment) {
    }

    /**
     * 新的一页开始（{@code bop}）。
     *
     * @param counters    TeX 的 10 个页计数器
     * @param previousBop 上一页 {@code bop} 在文件中的字节偏移（首页为 -1）
     */
    default void handleBeginPage(int[] counters, int previousBop) {
    }

    /**
     * 一个 {@code \special}。
     *
     * @param x        当前点的水平坐标
     * @param y        当前点的垂直坐标
     * @param contents special 的原始内容字节
     */
    default void handleSpecial(int x, int y, byte[] contents) {
    }

    /**
     * 一段连续排版的字符（由 {@code eop}、字体切换或其它命令触发 flush）。
     *
     * @param fontNum 当前字体编号
     * @param chars   字符编码（TeX 字符码，不是 Unicode）
     */
    default void handleCharRun(int fontNum, int[] chars) {
    }

    /**
     * 一段字形（{@code set_glyphs} / XDV_GLYPHS）。
     *
     * @param fontNum 当前字体编号
     * @param glyphs  字形 ID
     * @param x       每个字形的绝对水平坐标（已经加上当前点 h）
     * @param y       每个字形的绝对垂直坐标（已经加上当前点 v）
     */
    default void handleGlyphRun(int fontNum, char[] glyphs, int[] x, int[] y) {
    }

    /**
     * 文本 + 字形（{@code set_text_and_glyphs} / XDV_TEXT_AND_GLYPHS）。
     *
     * @param fontNum 当前字体编号
     * @param text    这段文本（由 UTF-16 字符码解码）
     * @param width   这段内容的排版宽度
     * @param glyphs  字形 ID
     * @param x       每个字形的绝对水平坐标（已经加上当前点 h）
     * @param y       每个字形的绝对垂直坐标（已经加上当前点 v）
     */
    default void handleTextAndGlyphs(
            int fontNum, String text, int width, char[] glyphs, int[] x, int[] y) {
    }

    /**
     * 定义 native 字体（{@code define_native_font}）。
     *
     * <p>末尾四个参数对应 XDV 字体标志位里的可选字段：标志位没置起时为
     * {@code null}。它们都是 Rust 侧 {@code u32}，以 Java {@code int} 位模式传递，
     * 可能为负（见类注释）。
     *
     * @param name       字体名
     * @param fontNum    字体编号
     * @param size       字号（fixed-point 原始值）
     * @param faceIndex  字体集合内的 face 索引（u32）
     * @param colorRgba  颜色 RGBA（u32，可选）
     * @param extend     水平拉伸（u32 fixed-point，可选）
     * @param slant      倾斜（u32 fixed-point，可选）
     * @param embolden   加粗程度（u32 fixed-point，可选）
     */
    default void handleDefineNativeFont(
            String name,
            int fontNum,
            int size,
            int faceIndex,
            Integer colorRgba,
            Integer extend,
            Integer slant,
            Integer embolden) {
    }

    /**
     * 一条 rule（{@code set_rule} 或 {@code put_rule}）。
     *
     * @param x      当前点的水平坐标
     * @param y      当前点的垂直坐标
     * @param height 高度
     * @param width  宽度
     */
    default void handleRule(int x, int y, int height, int width) {
    }
}
