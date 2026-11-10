# xdvjni — `tectonic_xdv` 的 JNI 绑定

[`tectonic_xdv`](https://crates.io/crates/tectonic_xdv) 是 Tectonic 项目里用 Rust 写的
XDV/SPX 解析器。**xdvjni** 把它包装成 Java 库：native 层是一个 Rust `cdylib`
（`libxdvjni.so`），Java 层提供与 Rust API 一一对应的回调式接口，供 Java 工程
（例如 XDV → OFD 转换器 **xdviofdmx**）直接依赖，无需自己写 JNI 胶水代码。

- Rust 依赖：`tectonic_xdv 0.3`、`jni 0.22`
- Java 基线：**Java 17**（`javac --release 17`）
- 构建：Maven；`mvn verify` 一条命令完成 Rust 构建、Rust 测试、Java 编译、JNI 端到端测试
- 产物：自包含 jar（native 库打进 `xdvjni/native/`，加载时自动解压）

## 快速开始

```bash
# 构建 + 全部测试（Rust cargo test + Java JUnit 5）
mvn verify

# 安装到本地 Maven 仓库，供 xdivofdmx 等工程依赖
mvn install
```

在下游工程里（本地 `mvn install` 后用本地坐标；或经 JitPack 直接解析，
见下）：

```xml
<dependency>
  <groupId>com.github.xiedada05</groupId>
  <artifactId>xdvjni</artifactId>
  <version>0.1.0</version>
</dependency>
```

JitPack 路线（Linux x86_64，首次解析会现场构建，`jitpack.yml` 自动安装
Rust 工具链）：仓库 `<url>https://jitpack.io</url>`。其它平台请 clone 本仓库
`mvn install`，或取 Release 中的全平台 jar。

## 使用示例

### 一次性解析

```java
import java.nio.file.Files;
import java.nio.file.Path;
import xdvjni.FileType;
import xdvjni.Xdv;
import xdvjni.XdvEvents;

byte[] data = Files.readAllBytes(Path.of("doc.xdv"));

long consumed = Xdv.parse(data, new XdvEvents() {
    @Override
    public void handleHeader(FileType filetype, byte[] comment) {
        System.out.println("类型: " + filetype);           // XDV / SPX
    }

    @Override
    public void handleDefineNativeFont(String name, int fontNum, int size, int faceIndex,
                                       Integer colorRgba, Integer extend,
                                       Integer slant, Integer embolden) {
        System.out.println("字体: " + name + " #" + fontNum);
    }

    @Override
    public void handleRule(int x, int y, int height, int width) {
        System.out.printf("rule @ (%d,%d) %dx%d%n", x, y, width, height);
    }
});
System.out.println(consumed + " 字节已解析");
```

### 流式解析

```java
import java.io.InputStream;
import xdvjni.XdvEvents;
import xdvjni.XdvParser;

try (InputStream in = Files.newInputStream(Path.of("doc.xdv"));
     XdvParser parser = new XdvParser(events)) {
    byte[] chunk = new byte[4096];
    byte[] pending = new byte[0];      // 上一次没被消耗的尾部字节
    int n;
    while ((n = in.read(chunk)) > 0) {
        // 把 pending 与本次读到的数据拼起来再喂（见下文“feed 契约”）
        byte[] input = concat(pending, chunk, n);
        int consumed = parser.feed(input);
        pending = Arrays.copyOfRange(input, consumed, input.length);
    }
    parser.finish();                   // 数据被截断时抛 XdvException
    System.out.println("总字节数: " + parser.currentOffset());
}
```

### `feed` 契约

`XdvParser.feed(data[, off, len])` 返回**本次调用消耗的字节数**：

- 解析器只处理完整指令，返回值可能小于 `len`；未消耗的尾部字节必须保留，
  与后续数据拼接后再次喂入（或者直接累积到下一次 `feed`）。
- 返回 `0` 表示当前缓冲还不足以解析下一条指令，需要继续积累输入。
- 数据流结束时调用 `finish()` 校验 postamble/double postamble；不完整则抛
  `XdvException`。

### 命令行演示

```bash
mvn -q package
java -cp target/xdvjni-0.1.0.jar xdvjni.Xdvdump doc.xdv
```

输出格式对齐 Tectonic 官方 Rust 示例 `crates/xdv/examples/xdvdump.rs`：

```
file type: XDV
comment: xdvjni sample
new page: [1 0 0 0 0 0 0 0 0 0] -1
define native font: `NotoSerif` num=5 size=655360 faceIndex=0 color=Some(4286599584) extend=None slant=None embolden=None
glyphs font=5: [36, 37] (@ [0, 327680], [100, 100])
special: color=red (@ 655360,100)
text font=5: "Hello" width=327680 glyphs=[40, 41] (@ [655360, 720896], [100, 100])
rule W=400 H=200 @ 983040, 100
chars font=5: [72, 105] all_ascii_printable=true
new page: [2 0 0 0 0 0 0 0 0 0] 28
define native font: `NotoSansCJK` num=6 size=300000 faceIndex=2 color=None extend=Some(65536) slant=None embolden=Some(32768)
rule W=50 H=50 @ 0, 0
chars font=6: [88] all_ascii_printable=true
329 bytes parsed.
```

与官方示例的差异（均在 `Xdvdump` 类注释中说明）：

- 官方可用 `--seek` 先解析尾部（依赖 `process_with_seeks`），xdvjni 目前只暴露
  顺序解析 API，传 `--seek` 会报错退出；
- 本工具补打印了官方示例未覆盖的 `SET_TEXT_AND_GLYPHS` 事件（`text ...` 行）；
- `glyphs` 行补上了官方示例漏掉的右括号（官方源码在此处少一个 `)`）。

## Java API

| Java | 对应 Rust | 说明 |
| --- | --- | --- |
| `Xdv.parse(byte[] data, XdvEvents)` | `XdvParser::process` | 一次性解析，返回消耗字节数 |
| `Xdv.parse(byte[] data, int off, int len, XdvEvents)` | `XdvParser::process` | 解析缓冲区的一段 |
| `new XdvParser(XdvEvents)` | `XdvParser::new` | 流式解析器（`AutoCloseable`） |
| `XdvParser.feed(byte[])` / `feed(byte[], off, len)` | `XdvParser::parse` | 喂入数据，返回消耗字节数 |
| `XdvParser.currentOffset()` | `XdvParser::current_offset` | 当前字节偏移（`finish()` 后仍可用） |
| `XdvParser.isFinished()` | — | `finish()` 之后为 `true` |
| `XdvParser.finish()` | `XdvParser::finish` | 断言流已正常结束 |
| `XdvParser.close()` | Rust 侧 `Drop` | 幂等释放 native 句柄 |
| `XdvEvents` | `XdvEvents` trait | 8 个回调，均有空实现（与 trait 默认实现一致） |
| `FileType` | `FileType` | `XDV` / `SPX` |
| `XdvException` | `XdvError` | 运行时异常，消息含字节偏移 |

### 回调参数

| 回调 | 参数说明 |
| --- | --- |
| `handleHeader(FileType, byte[])` | `comment` 保持原始字节，是否为 UTF-8 由调用方判断 |
| `handleBeginPage(int[] counters, int previousBop)` | 10 个 TeX 页计数器；首页 `previousBop == -1` |
| `handleSpecial(int x, int y, byte[] contents)` | `\special` 的内容与位置 |
| `handleCharRun(int fontNum, int[] chars)` | TeX 字符码（不是 Unicode），由 `eop`/换字体等触发 flush |
| `handleGlyphRun(int fontNum, char[] glyphs, int[] x, int[] y)` | 字形 ID 与**绝对**坐标（已加当前点） |
| `handleTextAndGlyphs(int fontNum, String text, int width, char[] glyphs, int[] x, int[] y)` | 文本 + 字形；`text` 由 UTF-16 解码 |
| `handleDefineNativeFont(String name, int fontNum, int size, int faceIndex, Integer colorRgba, Integer extend, Integer slant, Integer embolden)` | 4 个可选项未置标志位时为 `null` |
| `handleRule(int x, int y, int height, int width)` | `set_rule` / `put_rule` 都走这里 |

**无符号字段**：Rust 的 `u32`（`faceIndex` 与 4 个可选的字体参数）以 Java `int` 的
位模式传递，可能为负；需要无符号值时用
`Integer.toUnsignedLong(x)` / `Integer.toUnsignedString(x)` 读取
（例如 `0xFF8051A0` 会以 `-8367712` 传入，无符号即 `4286599584`）。

## 错误映射

| 来源 | Java 侧表现 |
| --- | --- |
| `XdvError::Malformed(offset)` | `XdvException: malformed XDV/SPX input at byte offset N` |
| `XdvError::IllegalOpcode(op, offset)` | `XdvException: illegal opcode OP (0xXX) at byte offset N` |
| `XdvError::UnexpectedEndOfStream` | `XdvException: unexpected end of XDV/SPX stream (truncated input)` |
| `XdvError::FromUTF8(offset)` | `XdvException: invalid UTF-8 in XDV/SPX input at byte offset N` |
| `XdvError::FromUTF16(offset)` | `XdvException: invalid UTF-16 in XDV/SPX input at byte offset N` |
| 参数非法（`off`/`len` 为负） | `XdvException: invalid argument: ...`（Java 层同时会先抛 `IndexOutOfBoundsException`） |
| 句柄状态非法（重复 `finish`、`feed` 已结束的解析器） | `XdvException: invalid parser state: ...` |
| 解析器已 `close()` | `IllegalStateException`（Java 层直接拦截） |
| `XdvEvents` 回调抛出的异常 | **原样传播**给 `parse`/`feed` 调用方，不会被包装 |
| JNI 内部错误 / Rust panic | `java.lang.RuntimeException` |

完整错误信息同时保留了 Rust 侧的字节偏移，便于对 XDV 数据做定位排查。

## 线程模型

- 回调在**调用 `parse` / `feed` / `finish` 的那个线程**上同步执行（native 层用
  `attach_current_thread` 复用已有线程，不会额外起线程）。
- 同一个 `XdvParser` **非线程安全**：同一时刻只允许一个线程使用。不同解析器实例
  之间互不影响，可并行。
- 不要在回调里重入同一个解析器。
- 回调参数中的数组/字符串都是为本次事件新建的副本，可自由保存。
- `close()` 幂等；忘记 `close()` 时由 `java.lang.ref.Cleaner` 兜底释放 native 句柄。

## 平台与加载

- 已在 **Linux x86_64** 实测（Rust 1.98 + OpenJDK 25，`--release 17` 编译）。
- 加载策略（见 `native/../Native.java`）：
  1. 先 `System.loadLibrary("xdvjni")`，走 `java.library.path` / 系统库路径；
  2. 失败则从 classpath 资源 `xdvjni/native/<平台库文件名>` 解压到临时文件再
     `System.load` —— 这就是 jar 自包含的原因。
- 本地库文件名按平台区分：Linux `libxdvjni.so`、macOS `libxdvjni.dylib`、
  Windows `xdvjni.dll`（native 库不能跨平台复用，需各自编译）。
- JDK 24+ 运行时可能打印 “A restricted method in java.lang.System has been
  called … `System::loadLibrary`” 警告，属正常提示；可用
  `--enable-native-access=ALL-UNNAMED` 消除（Java 17 无此选项，故未写进 pom）。

## 跨平台构建

构建脚本本身是跨平台的：cargo 在三大平台输出各自的 cdylib
（`libxdvjni.so` / `libxdvjni.dylib` / `xdvjni.dll`），构建期用 Ant 的 os
条件自动选择当前平台的库名并拷入 jar。在目标平台执行同一条命令即可：

```bash
mvn verify        # Linux/macOS/Windows 通用（需 Rust 工具链 + JDK 17+）
mvn package       # 产出含当前平台本地库的自包含 jar
```

GitHub Actions（`.github/workflows/ci.yml`）在 ubuntu / macos / windows 三个
runner 上执行 `mvn verify` 并上传各自的自包含 jar；推 tag 时自动额外组装
**全平台 jar**（把三个平台的本地库合并进同一个 `xdvjni/native/` 目录）并发布
Release，产物命名：

- `xdvjni-<ver>-linux-x86_64.jar`
- `xdvjni-<ver>-macos.jar`
- `xdvjni-<ver>-windows-x86_64.jar`
- `xdvjni-<ver>-all.jar`（全平台）

本地手工组装全平台 jar：把各平台 jar 里的 `xdvjni/native/*` 收集到一个目录，
再以该目录构建：

```bash
mkdir prebuilt-natives
unzip -j -o xdvjni-<ver>-macos.jar   'xdvjni/native/*' -d prebuilt-natives
unzip -j -o xdvjni-<ver>-windows.jar 'xdvjni/native/*' -d prebuilt-natives
mvn package -Dnative.prebuilt.dir=$PWD/prebuilt-natives
```

`Native.java` 运行时按当前 OS 挑选 jar 内匹配的库文件，因此全平台 jar 在三大
平台均可直接使用。macOS / Windows 的 Rust 侧逻辑与 Linux 完全一致（纯
`tectonic_xdv` 解析，无平台相关调用），CI 矩阵会持续验证。

## 工程结构

```
xdvjni/
├── pom.xml                        # Maven：release=17；exec 插件调 cargo；native 库打进 jar
├── Cargo.toml                     # crate-type = ["cdylib", "rlib"]；jni 0.22；tectonic_xdv 0.3
├── native/src/
│   ├── lib.rs                     # JNI 导出函数 + 流式句柄生命周期
│   ├── events.rs                  # JavaEvents：XdvEvents trait 的 JNI 转发实现
│   ├── cache.rs                   # OnceLock 缓存类/JMethodID/GlobalRef
│   ├── sample.rs                  # 手造 XDV 样本生成器（Rust/Java 测试共用）
│   └── bin/gen_sample.rs          # 生成 target/test-classes/sample.xdv
├── native/tests/sample_test.rs    # 用真实 tectonic_xdv 验证样本事件序列 + 流式一致性
├── src/main/java/xdvjni/
│   ├── XdvEvents.java             # 8 个回调（default 空实现）
│   ├── FileType.java              # XDV / SPX
│   ├── XdvException.java          # RuntimeException
│   ├── Xdv.java                   # 一次性解析入口
│   ├── XdvParser.java             # 流式解析器（AutoCloseable + Cleaner）
│   ├── Native.java                # native 方法声明 + 本地库加载
│   └── Xdvdump.java               # 演示 CLI（对齐官方 xdvdump 输出）
└── src/test/java/xdvjni/XdvBindingTest.java   # JUnit 5 端到端（11 个用例）
```

## 测试

`mvn verify` 依次执行：

1. `cargo build --release`：产出 `target/cargo/release/libxdvjni.so`；
2. `cargo run --bin gen-sample`：生成 `target/test-classes/sample.xdv`；
3. `mvn` 编译 Java 17 源码，把 native 库拷进 `target/classes/xdvjni/native/`；
4. JUnit 5（11 个用例）：一次性解析完整事件序列、`off/len` 区间、流式分块一致性、
   截断/非法 opcode 报错、回调异常原样传播、`close()` 生命周期、CLI 输出逐字节比对；
5. `cargo test --release`：Rust 侧用真实 `tectonic_xdv` 校验同一样本的事件序列，
   并验证一次性/流式结果一致。

测试样本是逐字节手造的 XDV（两页、含 native 字体、char run、special、glyph run、
text+glyphs、rule、postamble），Rust 与 Java 两端断言的事件序列必须保持一致。

### 开发环境备注

Maven 会从 `~/.m2/repository` 找依赖。若该目录不可写（例如受限沙箱），可把本地仓库
指向工程内目录：

```bash
mvn -Dmaven.repo.local=$PWD/.mvnrepo verify   # .mvnrepo/ 已在 .gitignore 中
```

## License

MIT（与 `tectonic_xdv` 一致）。
