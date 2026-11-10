package xdvjni;

/**
 * XDV/SPX 文件类型，对应 Rust 侧 {@code tectonic_xdv::FileType}。
 *
 * <p>{@link #XDV} / {@link #SPX} 两个常量由 native 层以静态字段方式定位，
 * 调用方不要做任何影响类初始化的反射操作。
 */
public enum FileType {
    /** 传统 XDV（XeTeX device independent）格式。 */
    XDV,

    /** Tectonic 的 SPX（semantically-paginated XDV）格式。 */
    SPX
}
