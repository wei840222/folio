#!/usr/bin/env python3
import sys
import argparse
from PIL import Image

def compress_image(input_path, output_path, quality=85):
    try:
        img = Image.open(input_path)
        # Convert RGBA to RGB if necessary for JPEG
        if img.mode in ("RGBA", "P"):
            img = img.convert("RGB")
        img.save(output_path, "JPEG", quality=quality)
        print(f"Successfully compressed {input_path} to {output_path}")
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Compress image to JPG for Folio upload")
    parser.add_argument("input", help="Input image path")
    parser.add_argument("output", help="Output JPG path")
    parser.add_argument("--quality", type=int, default=85, help="JPG quality (1-100)")

    args = parser.parse_args()
    compress_image(args.input, args.output, args.quality)
