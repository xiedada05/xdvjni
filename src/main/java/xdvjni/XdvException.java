package xdvjni;

/**
 * 解析 XDV/SPX 数据失败时抛出的运行时异常。
 *
 * <p>native 层把 Rust 的 {@code tectonic_xdv::XdvError} 映射为本异常，消息中带有
 * 出错的字节偏移，例如：
 *
 * <ul>
 *   <li>{@code malformed XDV/SPX input at byte offset 123}</li>
 *   <li>{@code illegal opcode 250 (0xfa) at byte offset 456}</li>
 *   <li>{@code unexpected end of XDV/SPX stream (truncated input)}</li>
 *   <li>{@code invalid UTF-8 in XDV/SPX input at byte offset 78}</li>
 * </ul>
 *
 * <p>注意：{@code XdvEvents} 回调自身抛出的异常<b>不会</b>被包装成
 * {@code XdvException}，而是原样传播给调用方。
 */
public class XdvException extends RuntimeException {

    private static final long serialVersionUID = 1L;

    /**
     * 构造异常。
     *
     * @param message 错误消息（来自 native 层，含字节偏移信息）
     */
    public XdvException(String message) {
        super(message);
    }

    /**
     * 构造带原因的异常。
     *
     * @param message 错误消息
     * @param cause   原始异常
     */
    public XdvException(String message, Throwable cause) {
        super(message, cause);
    }

    /**
     * 构造带原因的异常。
     *
     * @param cause 原始异常
     */
    public XdvException(Throwable cause) {
        super(cause);
    }
}
