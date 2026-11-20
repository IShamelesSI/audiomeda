"""Builds the .ico files from the source images: python assets/make_icons.py

The sources already have transparency, plus faint near-invisible noise around
the logo. That noise is cleared, then the image is cropped to its content,
padded square, and saved at icon sizes (.ico) and as a 256px .png preview.
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
    img.resize((256, 256), Image.LANCZOS).save(HERE / dst.replace(".ico", ".png"))
    print(f"{dst}: {img.size[0]}px square, sizes {sizes}")


if __name__ == "__main__":
    build("app.webp", "app.ico", [16, 24, 32, 48, 64, 128, 256])
    build("tray.png", "tray.ico", [16, 20, 24, 32, 40, 48, 64])
