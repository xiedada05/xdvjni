#!/usr/bin/env bash
# assemble-universal.sh —— 本地组装全平台 jar（镜像 .github/workflows/ci.yml
# 的 universal-jar 任务：收集各平台 jar 内的 xdvjni/native/* 合并后重建）。
#
# 用法:
#   ./assemble-universal.sh JAR_OR_DIR...          # 单平台 jar 或其所在目录
#   ./assemble-universal.sh                        # 用 target/xdvjni-<版本>.jar
#
# 三平台 jar 生成方式（各在对应 OS 上）: mvn verify 或 mvn package
# 产物: target/xdvjni-<版本>.jar（含 libxdvjni.so / libxdvjni.dylib / xdvjni.dll）

set -euo pipefail

die() { echo "assemble-universal: $*" >&2; exit 2; }

cd -- "$(dirname -- "${BASH_SOURCE[0]}")"
version=$(mvn -B -q --no-transfer-progress -Dexpression=project.version \
  -DforceStdout help:evaluate) || die "读取 pom 版本失败"

# ---- 收集输入 jar ----
inputs=()
if [[ $# -gt 0 ]]; then
  for a in "$@"; do
    if [[ -d $a ]]; then
      mapfile -t found < <(find "$a" -maxdepth 1 -name '*.jar' | sort)
      ((${#found[@]})) || die "目录中没有 jar: $a"
      inputs+=("${found[@]}")
    elif [[ -f $a ]]; then
      inputs+=("$a")
    else
      die "输入不存在: $a"
    fi
  done
else
  jar="target/xdvjni-${version}.jar"
  [[ -f $jar ]] || die "缺省 jar 不存在: $jar（先 mvn package 或显式传入 jar 路径）"
  inputs+=("$jar")
fi

# ---- 提取各平台本地库（按文件名去重；同名不同内容报错）----
prebuilt=$(mktemp -d)
trap 'rm -rf -- "$prebuilt"' EXIT
for jar in "${inputs[@]}"; do
  echo "提取 $jar"
  unzip -j -o -q "$jar" 'xdvjni/native/*' -d "$prebuilt" \
    || die "jar 中没有 xdvjni/native/* : $jar"
done

echo "---- 本地库清单 ----"
ls -la "$prebuilt"
missing=()
for lib in libxdvjni.so libxdvjni.dylib xdvjni.dll; do
  [[ -f $prebuilt/$lib ]] || missing+=("$lib")
done
if ((${#missing[@]})); then
  echo "警告: 缺少 ${missing[*]} —— 对应平台的 jar 无法加载" >&2
fi

# ---- 用 prebuilt 目录重建 ----
echo "---- 组装全平台 jar ----"
mvn -B --no-transfer-progress -DskipTests \
  -Dnative.prebuilt.dir="$prebuilt" package

out="target/xdvjni-${version}.jar"
echo
echo "已生成: $out"
unzip -l "$out" 'xdvjni/native/*' || true
