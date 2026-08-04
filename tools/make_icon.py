"""将选定的 PNG 图标转换为多分辨率 Windows .ico 文件。

用法:
    python tools/make_icon.py <source.png> <output.ico>

处理流程:
1. 读取源图并转为 RGBA
2. 自动裁剪掉四周完全透明的边距，让图标内容充满画布
3. 等比缩放并居中放入正方形画布
4. 导出包含多个标准尺寸的 .ico
"""

from __future__ import annotations

import sys
from pathlib import Path

# 优先使用安装到 F:\CodeDependence\pylibs 的依赖
_VENDOR = Path(r"F:\CodeDependence\pylibs")
if _VENDOR.is_dir():
    sys.path.insert(0, str(_VENDOR))

from PIL import Image  # noqa: E402

# Windows 资源管理器与任务栏使用的标准图标尺寸
ICON_SIZES = [16, 24, 32, 48, 64, 128, 256]

# 低于该 alpha 值视为透明，用于裁剪边距
ALPHA_THRESHOLD = 8


def trim_transparent(img: Image.Image) -> Image.Image:
    """裁掉四周完全透明的边距。若图片不含透明区域则原样返回。"""
    alpha = img.getchannel("A")
    # point 生成掩码: alpha 大于阈值的像素为 255
    mask = alpha.point(lambda a: 255 if a > ALPHA_THRESHOLD else 0)
    bbox = mask.getbbox()
    if bbox is None or bbox == (0, 0, img.width, img.height):
        return img
    return img.crop(bbox)


def to_square(img: Image.Image) -> Image.Image:
    """等比缩放并居中放入透明正方形画布。"""
    side = max(img.width, img.height)
    canvas = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    canvas.paste(img, ((side - img.width) // 2, (side - img.height) // 2))
    return canvas


def main() -> int:
    if len(sys.argv) != 3:
        print(__doc__)
        return 2

    src = Path(sys.argv[1])
    dst = Path(sys.argv[2])

    if not src.is_file():
        print(f"ERROR: 源文件不存在: {src}")
        return 1

    img = Image.open(src).convert("RGBA")
    print(f"源图尺寸: {img.width}x{img.height}")

    alpha_range = img.getchannel("A").getextrema()
    print(f"alpha 范围: {alpha_range}")

    img = trim_transparent(img)
    print(f"裁剪后: {img.width}x{img.height}")

    img = to_square(img)
    print(f"方形画布: {img.width}x{img.height}")

    # 用最高质量重采样生成 256x256 基准图
    base = img.resize((256, 256), Image.LANCZOS)

    dst.parent.mkdir(parents=True, exist_ok=True)
    base.save(dst, format="ICO", sizes=[(s, s) for s in ICON_SIZES])

    print(f"已生成: {dst} ({dst.stat().st_size} 字节)")
    print(f"包含尺寸: {ICON_SIZES}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
