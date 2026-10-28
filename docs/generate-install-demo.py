"""Generate only synthetic install-smoke evidence (requires Pillow and a TTF)."""

from pathlib import Path
import sys

from PIL import Image, ImageDraw, ImageFont


root = Path(__file__).resolve().parents[1]
destination = root / "demo" / "evidence"
destination.mkdir(parents=True, exist_ok=True)
font = ImageFont.truetype(sys.argv[1], 32)
image = Image.new("RGB", (1000, 220), "white")
draw = ImageDraw.Draw(image)
draw.text((32, 28), "Synthetic install smoke evidence", font=font, fill="black")
draw.text((32, 110), "demo@example.com", font=font, fill="black")
image.save(destination / "screenshot.png")
(destination / "synthetic.log").write_text(
    "Synthetic installation smoke fixture; no real personal data.\n"
    "email=demo@example.com\n"
    "payment_card=4242 4242 4242 4242\n",
    encoding="utf-8",
)
