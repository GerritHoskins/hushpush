"""Generate a screenshot with synthetic values; pass a local monospace TTF path."""
from pathlib import Path
import sys
from PIL import Image, ImageDraw, ImageFont
image = Image.new("RGB", (900, 170), "white")
draw = ImageDraw.Draw(image)
font = ImageFont.truetype(sys.argv[1], 30)
draw.text((24, 20), "demo@example.com", font=font, fill="black")
draw.text((24, 84), "4242 4242 4242 4242", font=font, fill="black")
image.save(Path(__file__).with_name("demo.png"))
