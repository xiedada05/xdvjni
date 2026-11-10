package xdvjni;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;

/**
 * 演示用命令行工具：解析 XDV/SPX 文件并打印事件，输出格式对齐 Tectonic 官方
 * Rust 示例 {@code crates/xdv/examples/xdvdump.rs}。
 *
 * <pre>{@code
 * java -cp xdvjni.jar xdvjni.Xdvdump doc.xdv
 * }</pre>
 *
 * <p>与官方示例的差异：
 *
 * <ul>
 *   <li>官方示例可用 {@code --seek} 先解析尾部；xdvjni 目前只暴露顺序解析 API，
 *       传 {@code --seek} 会报错退出。</li>
 *   <li>官方示例没有覆盖 {@code handle_text_and_glyphs}，本工具为它额外打印一行
 *       {@code text ...}（XDV 的 SET_TEXT_AND_GLYPHS 事件）。</li>
 * </ul>
 */
public final class Xdvdump {

    private Xdvdump() {
    }

    /**
     * 程序入口。
     *
     * @param args {@code [--seek] <file>}
     */
    public static void main(String[] args) {
        boolean seek = false;
        String pathArg = null;
        for (String arg : args) {
            if ("--seek".equals(arg)) {
                seek = true;
            } else if (arg.startsWith("-")) {
                System.err.println("error: 未知选项 \"" + arg + "\"");
                usage();
                System.exit(2);
                return;
            } else if (pathArg == null) {
                pathArg = arg;
            } else {
                usage();
                System.exit(2);
                return;
            }
        }
        if (pathArg == null) {
            usage();
            System.exit(2);
            return;
        }
        if (seek) {
            System.err.println("error: --seek 未实现（xdvjni 只提供顺序解析 API）");
            System.exit(2);
            return;
        }

        Path path = Path.of(pathArg);
        byte[] data;
        try {
            data = Files.readAllBytes(path);
        } catch (IOException e) {
            System.err.println("error: could not open \"" + path + "\": " + e.getMessage());
            System.exit(1);
            return;
        }

        Stats stats = new Stats();
        long nBytes;
        try {
            nBytes = Xdv.parse(data, stats);
        } catch (XdvException e) {
            System.err.println("error: failed to parse \"" + path + "\": " + e.getMessage());
            System.exit(1);
            return;
        }

        System.out.println(nBytes + " bytes parsed.");
    }

    private static void usage() {
        System.err.println("usage: xdvdump [--seek] <file.xdv|file.spx>");
    }

    /** 打印格式与官方 Rust 示例保持一致的事件处理器。 */
    private static final class Stats implements XdvEvents {

        @Override
        public void handleHeader(FileType filetype, byte[] comment) {
            System.out.println("file type: " + filetype);
            System.out.println("comment: " + new String(comment, StandardCharsets.UTF_8));
        }

        @Override
        public void handleDefineNativeFont(
                String name,
                int fontNum,
                int size,
                int faceIndex,
                Integer colorRgba,
                Integer extend,
                Integer slant,
                Integer embolden) {
            System.out.println(
                    "define native font: `"
                            + name
                            + "` num="
                            + fontNum
                            + " size="
                            + size
                            + " faceIndex="
                            + Integer.toUnsignedString(faceIndex)
                            + " color="
                            + debugOption(colorRgba)
                            + " extend="
                            + debugOption(extend)
                            + " slant="
                            + debugOption(slant)
                            + " embolden="
                            + debugOption(embolden));
        }

        @Override
        public void handleBeginPage(int[] counters, int previousBop) {
            StringBuilder sb = new StringBuilder("new page: [");
            for (int i = 0; i < counters.length; i++) {
                if (i > 0) {
                    sb.append(' ');
                }
                sb.append(counters[i]);
            }
            sb.append("] ").append(previousBop);
            System.out.println(sb);
        }

        @Override
        public void handleSpecial(int x, int y, byte[] contents) {
            System.out.println(
                    "special: " + new String(contents, StandardCharsets.UTF_8) + " (@ " + x + "," + y + ")");
        }

        @Override
        public void handleCharRun(int fontNum, int[] chars) {
            boolean allAsciiPrintable = true;
            for (int c : chars) {
                if (c <= 0x20 || c >= 0x7F) {
                    allAsciiPrintable = false;
                    break;
                }
            }
            System.out.println(
                    "chars font="
                            + fontNum
                            + ": "
                            + Arrays.toString(chars)
                            + " all_ascii_printable="
                            + allAsciiPrintable);
        }

        @Override
        public void handleGlyphRun(int fontNum, char[] glyphs, int[] x, int[] y) {
            System.out.println(
                    "glyphs font="
                            + fontNum
                            + ": "
                            + charArrayToString(glyphs)
                            + " (@ "
                            + Arrays.toString(x)
                            + ", "
                            + Arrays.toString(y)
                            + ")");
        }

        @Override
        public void handleTextAndGlyphs(
                int fontNum, String text, int width, char[] glyphs, int[] x, int[] y) {
            System.out.println(
                    "text font="
                            + fontNum
                            + ": \""
                            + text
                            + "\" width="
                            + width
                            + " glyphs="
                            + charArrayToString(glyphs)
                            + " (@ "
                            + Arrays.toString(x)
                            + ", "
                            + Arrays.toString(y)
                            + ")");
        }

        @Override
        public void handleRule(int x, int y, int height, int width) {
            System.out.println("rule W=" + width + " H=" + height + " @ " + x + ", " + y);
        }

        /** 模仿 Rust 的 {@code {:?}}：{@code Some(u32)} / {@code None}。 */
        private static String debugOption(Integer value) {
            return value == null ? "None" : "Some(" + Integer.toUnsignedString(value) + ")";
        }

        /** 模仿 Rust 的 {@code Vec<u16>} 调试输出，例如 {@code [36, 37]}。 */
        private static String charArrayToString(char[] chars) {
            StringBuilder sb = new StringBuilder("[");
            for (int i = 0; i < chars.length; i++) {
                if (i > 0) {
                    sb.append(", ");
                }
                sb.append((int) chars[i]);
            }
            return sb.append(']').toString();
        }
    }
}
