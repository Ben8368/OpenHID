"""Generate OpenHID artwork from original geometry. Requires Pillow."""
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent

def symbol(prism=False, transparent=False):
    image = Image.new("RGBA", (1024, 1024), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    if not transparent:
        draw.rounded_rectangle((40, 40, 984, 984), radius=220, fill="#132B3B")
    ink = "#C6B7FF" if prism else "#65D9CB"
    # An H with three device endpoints: two stems joined by a signal path.
    draw.rounded_rectangle((250, 240, 370, 784), radius=60, fill=ink)
    draw.rounded_rectangle((654, 240, 774, 784), radius=60, fill=ink)
    draw.rounded_rectangle((310, 452, 714, 572), radius=60, fill=ink)
    for x, y in ((310, 240), (714, 240), (714, 784)):
        draw.ellipse((x-78, y-78, x+78, y+78), fill="#F3F8FC")
        draw.ellipse((x-27, y-27, x+27, y+27), fill="#132B3B")
    return image

def main():
    icon = ROOT / "icon"
    icon.mkdir(exist_ok=True)
    for variant, prism in (("openhid", False), ("openhid-prism", True)):
        image = symbol(prism)
        image.save(icon / f"{variant}.png")
        image.save(icon / f"{variant}.ico", sizes=[(s,s) for s in (16,24,32,48,64,128,256)])
        document = icon / f"{variant}.icon"
        (document / "Assets").mkdir(parents=True, exist_ok=True)
        asset = "OpenHID-1.png" if prism else "OpenHID.png"
        symbol(prism, transparent=True).save(document / "Assets" / asset)
    for size in (16,32,48,64,128,256,512):
        symbol().resize((size,size), Image.Resampling.LANCZOS).save(icon / f"openhid-{size}.png")
    symbol().save(ROOT.parent / "crates/openhid-desktop/icon/AppIcon.icns", sizes=[(s,s) for s in (16,32,64,128,256,512,1024)])
    tray_dir = ROOT.parent / "crates/openhid-agent/assets"
    for filename, ink in (("tray-icon@2x.png", "#132B3B"), ("tray-icon-white@2x.png", "#FFFFFF"), ("tray-icon-prism@2x.png", "#C6B7FF")):
        tray = symbol(transparent=True)
        alpha = tray.getchannel("A")
        tray = Image.new("RGBA", tray.size, ink)
        tray.putalpha(alpha)
        tray.resize((36,36), Image.Resampling.LANCZOS).save(tray_dir / filename)
    background = Image.new("RGB", (760,480), "#132B3B")
    draw = ImageDraw.Draw(background)
    draw.line((335,250,425,250), fill="#65D9CB", width=8)
    draw.line((405,230,425,250,405,270), fill="#65D9CB", width=8)
    background.save(ROOT / "bg/openhid-dmg.png")

if __name__ == "__main__":
    main()
