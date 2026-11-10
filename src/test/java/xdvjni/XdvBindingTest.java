package xdvjni;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertSame;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import org.junit.jupiter.api.Test;

/**
 * xdvjni 端到端测试：在真实 JVM 中通过 JNI 解析 {@code native/src/sample.rs} 生成的
 * 样本文件，断言完整事件序列、流式解析、错误路径、回调异常透传与 CLI 输出。
 *
 * <p>样本由 Maven 的 {@code gen-sample} 执行步骤生成到
 * {@code target/test-classes/sample.xdv}；Rust 侧 {@code cargo test} 用同一份生成器
 * 校验事件序列，两边断言必须保持一致。
 */
class XdvBindingTest {

    /** 与 native/src/sample.rs 的注释以及 Rust 测试对应的事件文本形式。 */
    private static final List<String> EXPECTED_EVENTS =
            List.of(
                    "header XDV \"xdvjni sample\"",
                    "beginPage [1, 0, 0, 0, 0, 0, 0, 0, 0, 0] prev=-1",
                    "font \"NotoSerif\" num=5 size=655360 face=0 color=4286599584"
                            + " extend=null slant=null embolden=null",
                    "glyphs num=5 ids=[36, 37] x=[0, 327680] y=[100, 100]",
                    "special (655360,100) \"color=red\"",
                    "text num=5 \"Hello\" width=327680 ids=[40, 41]"
                            + " x=[655360, 720896] y=[100, 100]",
                    "rule (983040,100) h=200 w=400",
                    "chars num=5 [72, 105]",
                    "beginPage [2, 0, 0, 0, 0, 0, 0, 0, 0, 0] prev=28",
                    "font \"NotoSansCJK\" num=6 size=300000 face=2 color=null"
                            + " extend=65536 slant=null embolden=32768",
                    "rule (0,0) h=50 w=50",
                    "chars num=6 [88]");

    /** 读取 Maven 构建阶段生成的样本文件。 */
    private static byte[] sample() throws IOException {
        try (InputStream in = XdvBindingTest.class.getResourceAsStream("/sample.xdv")) {
            assertNotNull(in, "sample.xdv 缺失：应由 mvn 的 gen-sample 执行步骤生成");
            return in.readAllBytes();
        }
    }

    // ------------------------------------------------------------------
    // 一次性解析
    // ------------------------------------------------------------------

    @Test
    void parseProducesExpectedEvents() throws IOException {
        byte[] data = sample();
        Recorder recorder = new Recorder();

        long consumed = Xdv.parse(data, recorder);

        assertEquals(data.length, consumed, "一次性解析应消耗全部字节");
        assertEquals(EXPECTED_EVENTS, recorder.events);
    }

    @Test
    void parseHonoursOffsetAndLength() throws IOException {
        byte[] data = sample();
        byte[] padded = new byte[data.length + 3];
        System.arraycopy(data, 0, padded, 3, data.length);
        Recorder recorder = new Recorder();

        long consumed = Xdv.parse(padded, 3, data.length, recorder);

        assertEquals(data.length, consumed);
        assertEquals(EXPECTED_EVENTS, recorder.events);
    }

    // ------------------------------------------------------------------
    // 流式解析
    // ------------------------------------------------------------------

    @Test
    void streamingFeedMatchesOneShot() throws IOException {
        byte[] data = sample();

        Recorder oneShot = new Recorder();
        Xdv.parse(data, oneShot);

        Recorder streaming = new Recorder();
        long finalOffset;
        try (XdvParser parser = new XdvParser(streaming)) {
            // 按 7 字节分块喂入，遵循 feed 的“未消耗字节需重新喂入”契约。
            byte[] buffer = new byte[data.length];
            int bufferLen = 0;
            int next = 0;

            while (next < data.length || bufferLen > 0) {
                if (next < data.length) {
                    int n = Math.min(7, data.length - next);
                    System.arraycopy(data, next, buffer, bufferLen, n);
                    bufferLen += n;
                    next += n;
                }

                int consumed = parser.feed(buffer, 0, bufferLen);
                assertTrue(consumed <= bufferLen, "解析器消耗的字节数不应超过可用字节数");
                System.arraycopy(buffer, consumed, buffer, 0, bufferLen - consumed);
                bufferLen -= consumed;

                if (consumed == 0 && next >= data.length) {
                    throw new AssertionError(
                            "解析器在还剩 " + bufferLen + " 字节时停滞，无法继续");
                }
            }

            assertFalse(parser.isFinished(), "未调用 finish() 前不应报告已结束");
            assertEquals(data.length, parser.currentOffset(), "解析结束时偏移应等于文件长度");

            parser.finish();
            assertTrue(parser.isFinished(), "finish() 之后应报告已结束");
            finalOffset = parser.currentOffset();
        }

        assertEquals(data.length, finalOffset, "finish() 之后 currentOffset() 仍应可用");
        assertEquals(oneShot.events, streaming.events);
    }

    // ------------------------------------------------------------------
    // 错误路径
    // ------------------------------------------------------------------

    @Test
    void truncatedStreamThrowsXdvException() throws IOException {
        byte[] data = sample();
        byte[] cut = Arrays.copyOf(data, data.length - 6);

        XdvException error = assertThrows(XdvException.class, () -> Xdv.parse(cut, new Recorder()));
        assertTrue(
                error.getMessage().contains("end of XDV/SPX stream"),
                "截断错误消息应说明流意外结束: " + error.getMessage());
    }

    @Test
    void illegalOpcodeThrowsXdvException() throws IOException {
        byte[] data = sample();
        // 偏移 28 是第一页的 bop opcode；255 不是任何合法 XDV opcode。
        data[28] = (byte) 0xFF;

        XdvException error = assertThrows(XdvException.class, () -> Xdv.parse(data, new Recorder()));
        assertTrue(
                error.getMessage().contains("illegal opcode 255"),
                "非法 opcode 错误消息应包含 opcode 值: " + error.getMessage());
    }

    @Test
    void streamingTruncatedFinishThrows() throws IOException {
        byte[] data = sample();
        byte[] cut = Arrays.copyOf(data, data.length - 6);

        try (XdvParser parser = new XdvParser(new Recorder())) {
            int consumed = parser.feed(cut);
            assertTrue(consumed > 0, "至少应消耗掉完整指令");
            assertTrue(consumed <= cut.length, "消耗字节数不应超过可用字节数");
            if (consumed < cut.length) {
                // 尾部不足一条完整指令：再喂一次应返回 0（需要更多数据）。
                assertEquals(
                        0,
                        parser.feed(cut, consumed, cut.length - consumed),
                        "不完整尾部不应被消耗");
            }
            assertThrows(XdvException.class, parser::finish);
        }
    }

    @Test
    void callbackExceptionPropagatesUnchanged() throws IOException {
        byte[] data = sample();
        RuntimeException boom = new IllegalStateException("boom from callback");

        XdvEvents failing =
                new XdvEvents() {
                    @Override
                    public void handleHeader(FileType filetype, byte[] comment) {
                        throw boom;
                    }
                };

        RuntimeException thrown =
                assertThrows(RuntimeException.class, () -> Xdv.parse(data, failing));
        assertSame(boom, thrown, "回调抛出的异常必须原样传播，不能被包装");
    }

    // ------------------------------------------------------------------
    // 句柄生命周期
    // ------------------------------------------------------------------

    @Test
    void closeIsIdempotentAndInvalidatesHandle() throws IOException {
        byte[] data = sample();
        XdvParser parser = new XdvParser(new Recorder());
        parser.feed(data, 0, 8);
        parser.close();
        parser.close(); // 幂等

        assertThrows(IllegalStateException.class, () -> parser.feed(data));
        assertThrows(IllegalStateException.class, parser::currentOffset);
        assertThrows(IllegalStateException.class, parser::isFinished);
        assertThrows(IllegalStateException.class, parser::finish);
    }

    @Test
    void finishAfterFinishThrows() throws IOException {
        byte[] data = sample();
        try (XdvParser parser = new XdvParser(new Recorder())) {
            assertEquals(data.length, parser.feed(data));
            parser.finish();
            assertThrows(XdvException.class, parser::finish);
        }
    }

    // ------------------------------------------------------------------
    // CLI 输出（对齐官方 Rust xdvdump 格式）
    // ------------------------------------------------------------------

    @Test
    void xdvdumpPrintsOfficialFormat() throws IOException {
        byte[] data = sample();
        Path tmp = Files.createTempFile("xdvjni-dump-", ".xdv");
        try {
            Files.write(tmp, data);

            PrintStream original = System.out;
            ByteArrayOutputStream captured = new ByteArrayOutputStream();
            String actual;
            try {
                System.setOut(new PrintStream(captured, true, StandardCharsets.UTF_8));
                Xdvdump.main(new String[] {tmp.toString()});
            } finally {
                System.setOut(original);
            }
            // Windows 上 println 产生 \r\n，对比前归一化行尾
            actual = captured.toString(StandardCharsets.UTF_8).replace("\r\n", "\n");

            String expected =
                    "file type: XDV\n"
                            + "comment: xdvjni sample\n"
                            + "new page: [1 0 0 0 0 0 0 0 0 0] -1\n"
                            + "define native font: `NotoSerif` num=5 size=655360 faceIndex=0"
                            + " color=Some(4286599584) extend=None slant=None embolden=None\n"
                            + "glyphs font=5: [36, 37] (@ [0, 327680], [100, 100])\n"
                            + "special: color=red (@ 655360,100)\n"
                            + "text font=5: \"Hello\" width=327680 glyphs=[40, 41]"
                            + " (@ [655360, 720896], [100, 100])\n"
                            + "rule W=400 H=200 @ 983040, 100\n"
                            + "chars font=5: [72, 105] all_ascii_printable=true\n"
                            + "new page: [2 0 0 0 0 0 0 0 0 0] 28\n"
                            + "define native font: `NotoSansCJK` num=6 size=300000 faceIndex=2"
                            + " color=None extend=Some(65536) slant=None embolden=Some(32768)\n"
                            + "rule W=50 H=50 @ 0, 0\n"
                            + "chars font=6: [88] all_ascii_printable=true\n"
                            + data.length
                            + " bytes parsed.\n";

            assertEquals(expected, actual);
        } finally {
            Files.deleteIfExists(tmp);
        }
    }

    // ------------------------------------------------------------------
    // 事件记录器
    // ------------------------------------------------------------------

    /**
     * 把事件渲染成稳定文本，便于与 Rust 侧断言逐项对齐。
     *
     * <p>{@code u32} 字段按无符号十进制打印，便于阅读真实颜色值。
     */
    private static final class Recorder implements XdvEvents {

        private final List<String> events = new ArrayList<>();

        @Override
        public void handleHeader(FileType filetype, byte[] comment) {
            events.add(
                    "header "
                            + filetype
                            + " \""
                            + new String(comment, StandardCharsets.UTF_8)
                            + "\"");
        }

        @Override
        public void handleBeginPage(int[] counters, int previousBop) {
            events.add("beginPage " + Arrays.toString(counters) + " prev=" + previousBop);
        }

        @Override
        public void handleSpecial(int x, int y, byte[] contents) {
            events.add(
                    "special ("
                            + x
                            + ","
                            + y
                            + ") \""
                            + new String(contents, StandardCharsets.UTF_8)
                            + "\"");
        }

        @Override
        public void handleCharRun(int fontNum, int[] chars) {
            events.add("chars num=" + fontNum + " " + Arrays.toString(chars));
        }

        @Override
        public void handleGlyphRun(int fontNum, char[] glyphs, int[] x, int[] y) {
            events.add(
                    "glyphs num="
                            + fontNum
                            + " ids="
                            + ints(glyphs)
                            + " x="
                            + Arrays.toString(x)
                            + " y="
                            + Arrays.toString(y));
        }

        @Override
        public void handleTextAndGlyphs(
                int fontNum, String text, int width, char[] glyphs, int[] x, int[] y) {
            events.add(
                    "text num="
                            + fontNum
                            + " \""
                            + text
                            + "\" width="
                            + width
                            + " ids="
                            + ints(glyphs)
                            + " x="
                            + Arrays.toString(x)
                            + " y="
                            + Arrays.toString(y));
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
            events.add(
                    "font \""
                            + name
                            + "\" num="
                            + fontNum
                            + " size="
                            + size
                            + " face="
                            + Integer.toUnsignedString(faceIndex)
                            + " color="
                            + unsigned(colorRgba)
                            + " extend="
                            + unsigned(extend)
                            + " slant="
                            + unsigned(slant)
                            + " embolden="
                            + unsigned(embolden));
        }

        @Override
        public void handleRule(int x, int y, int height, int width) {
            events.add("rule (" + x + "," + y + ") h=" + height + " w=" + width);
        }

        private static String unsigned(Integer value) {
            return value == null ? "null" : Integer.toUnsignedString(value);
        }

        private static String ints(char[] values) {
            int[] copy = new int[values.length];
            for (int i = 0; i < values.length; i++) {
                copy[i] = values[i];
            }
            return Arrays.toString(copy);
        }
    }

    // ---- 下列断言用于确保持久化字节数组没有被 native 层复用（防御性检查） ----

    @Test
    void callbackArraysAreIndependentCopies() throws IOException {
        byte[] data = sample();
        List<byte[]> headerComments = new ArrayList<>();
        List<int[]> charRuns = new ArrayList<>();

        XdvEvents collecting =
                new XdvEvents() {
                    @Override
                    public void handleHeader(FileType filetype, byte[] comment) {
                        headerComments.add(comment);
                    }

                    @Override
                    public void handleCharRun(int fontNum, int[] chars) {
                        charRuns.add(chars);
                    }
                };

        Xdv.parse(data, collecting);

        assertEquals(1, headerComments.size());
        assertArrayEquals("xdvjni sample".getBytes(StandardCharsets.UTF_8), headerComments.get(0));
        assertEquals(2, charRuns.size());
        assertArrayEquals(new int[] {72, 105}, charRuns.get(0));
        assertArrayEquals(new int[] {88}, charRuns.get(1));
    }
}
