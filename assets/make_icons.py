"""Builds every icon from the two source images: python assets/make_icons.py

Writes assets/{app,tray}.{ico,png} (embedded in the engine exe), the UI window icons in
ui/src-tauri/icons and the header logo in ui/static/logo.png. Rerun it after changing a source.

The sources already have transparency, plus faint near-invisible noise around
the logo. That noise is cleared, then the image is cropped to its content,
padded square, and saved at icon sizes (.ico) and as a full-size square .png.
"""
from pathlib import Path
from PIL import Image

HERE = Path(__file__).parent
NOISE = 24  # alpha at or below this (out of 255) counts as background


def square(img: Image.Image) -> Image.Image:
    img = img.convert("RGBA")
    img.putalpha(img.getchannel("A").point(lambda a: 0 if a <= NOISE else a))
    img = img.crop(img.getchannel("A").getbbox())
    side = max(img.size)
    canvas = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    canvas.paste(img, ((side - img.width) // 2, (side - img.height) // 2))
    return canvas


def build(src: str, dst: str, sizes):
    img = square(Image.open(HERE / "source" / src))
    img.save(HERE / dst, sizes=[(s, s) for s in sizes])
    img.save(HERE / dst.replace(".ico", ".png"))  # full-size square, e.g. for `tauri icon`
    print(f"{dst}: {img.size[0]}px square, sizes {sizes}")


def ui_icons():
    img = Image.open(HERE / "app.png")
    ui = HERE.parent / "ui"
    icons = ui / "src-tauri" / "icons"
    for name, side in [("32x32.png", 32), ("128x128.png", 128), ("128x128@2x.png", 256), ("icon.png", 512)]:
        img.resize((side, side), Image.LANCZOS).save(icons / name)
    img.save(icons / "icon.ico", sizes=[(s, s) for s in [16, 24, 32, 48, 64, 256]])
    img.save(ui / "static" / "logo.png")
    print("ui icons and logo written")


if __name__ == "__main__":
    build("app.webp", "app.ico", [16, 24, 32, 48, 64, 128, 256])
    build("tray.png", "tray.ico", [16, 20, 24, 32, 40, 48, 64])
    ui_icons()
