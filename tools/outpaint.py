#!/usr/bin/env python3
"""Paint the wide pictures the remastered look of the ANSTOSS 3 replica draws on.

The app draws every remastered dialog on a 16:9 picture made from the game's
4:3 artwork. Most dialogs of the new-game screens share one artwork: it goes
to the left, cleaned of the interface painted into it, and carries on to the
right edge, where the remastered interface goes. Dialogs with pictures of
their own keep them in the middle, cleaned, and carried on to both sides;
scenes only grow to the sides. SDXL inpainting
repaints what the masks mark — the painted interface and the new space — and
everything else of the original is laid back on top, untouched.

It reads the pictures and masks the app exports (``anstoss-3-1
--export-backgrounds``, run here automatically when they are missing) and
writes ``~/.cache/anstoss-3.1/wide/<name>.png``, which the app picks up.

    tools/.venv/bin/python tools/outpaint.py [--force] [--seed N] [names...]
"""

import argparse
import os
import subprocess
import sys
from pathlib import Path

import numpy as np
import torch
from diffusers import AutoencoderKL, AutoPipelineForInpainting
from PIL import Image, ImageDraw, ImageFilter, ImageOps

MODEL = "diffusers/stable-diffusion-xl-1.0-inpainting-0.1"
# The stock SDXL VAE overflows in half precision and decodes to black.
VAE = "madebyollin/sdxl-vae-fp16-fix"

CANVAS = (1920, 1080)
# The 4:3 picture on the canvas.
ART = (1440, 1080)
# SDXL works at about a megapixel, in multiples of 8: the picture at 1024×768
# in a frame wide enough to still reach the canvas edges once scaled to ART.
GEN_ART = (1024, 768)
GEN = (1376, 768)
# How far a repainted area reaches into the original, so it blends in.
OVERLAP = 24
# Width of the soft seam where the original is laid back over the result.
SEAM = 40
# The shared artwork goes to the left; the others stay in the middle.
LEFT = {"base"}

PROMPTS = {
    "base": "very light grey textured paper background, soft smudges and faint shadows, "
    "a few thin drawn arcs, calm empty space",
    "256": "abstract collage, orange and blue blurred computer keyboard and football, "
    "light grey paper texture",
    "95": "light grey paper texture, faded photo of a cheering football team, blue accents",
    "381": "dreamy collage of football players in action, pale blue tint, motion blur, "
    "white haze, light paper texture",
    "483": "bright rendered interior of a football manager's office, warm orange and yellow "
    "walls, polished wooden floor, potted green plants, soft daylight",
    "504": "rendered interior of a grand oval office of a football club president, cream "
    "walls with classical white mouldings, wooden furniture, carpet, warm light",
}
PROMPTS.update({
    # Screens covered by their interface get calm paper in the colours of
    # their pictures.
    "99": "very light grey textured paper background, soft warm red and orange watercolour "
    "smudges, calm empty space",
    "103": "very light grey textured paper background, soft teal and cyan streaks of light, "
    "calm empty space",
    "147": "very light grey textured paper background, soft smudges with a faint green tint, "
    "calm empty space",
    "302": "faded collage of an old man with a flat cap and round glasses, a computer "
    "keyboard and music notes, light grey paper texture, calm empty space",
    # The week screen, every one of them on the picture without panels.
    "1000": "pale blue tinted photo collage of goalkeepers diving for a football, goal net, "
    "light grey paper texture, motion blur",
})
# Pictures whose edge holds something that must not be repeated — the week's
# column of weekdays — start their new space from a smooth fill, not a mirror.
SMOOTH_START = {"1000"}
# Scenes that go on beyond the picture need other things kept out of them.
NEGATIVES = {
    "483": "text, letters, words, logo, watermark, signature, person, people, face, "
    "black bars, letterbox, duplicate, deformed, blurry, haze",
}
NEGATIVES["504"] = NEGATIVES["483"]
STYLE = ", soft pastel colours, 2000s sports video game menu artwork, high detail, seamless"
NEGATIVE = (
    "text, letters, words, logo, watermark, signature, user interface, frame, border, "
    "window, button, radio button, icons, symbols, chain, ball, small objects, "
    "stripes, bars, horizontal lines, boxes, pencil, stick, "
    "black bars, letterbox, duplicate, deformed, blurry, haze"
)
for name in ("99", "103", "147"):
    NEGATIVES[name] = NEGATIVE + ", person, people, face, hand, body, football, ball, pitch, grass"
# Seeds picked by eye where the default one invented something odd.
SEEDS: dict[str, int] = {"base": 7, "256": 23, "95": 42, "103": 42, "1000": 11}
# Pictures that came out darker than the light paper of the other screens,
# and how far towards white they are lightened.
LIGHTEN: dict[str, float] = {"103": 0.45}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("names", nargs="*", help="pictures to paint: base, 256, 95, 99, 103, 147, 302, 381, 483, 504, 1000 (default: all)")
    parser.add_argument("--force", action="store_true", help="redo existing results")
    parser.add_argument("--seed", type=int, help="seed for every picture (default: SEEDS, else 7)")
    parser.add_argument("--steps", type=int, default=40)
    parser.add_argument("--out", type=Path, help="write here instead of the app's cache")
    args = parser.parse_args()

    cache = cache_dir()
    sources = cache / "sources"
    if not any(sources.glob("*_mask.png")):
        export(sources)
    wide = args.out or cache / "wide"
    wide.mkdir(parents=True, exist_ok=True)

    names = sorted(path.name.removesuffix("_mask.png") for path in sources.glob("*_mask.png"))
    todo = [
        name
        for name in names
        if (not args.names or name in args.names)
        and (args.force or not (wide / f"{name}.png").exists())
    ]
    if not todo:
        print("Nothing to do; pass --force to paint them again.")
        return

    pipe = load_pipeline()
    for name in todo:
        print(f"{name}: painting …", flush=True)
        art = Image.open(sources / f"{name}.png").convert("RGB")
        mask = Image.open(sources / f"{name}_mask.png").convert("L")
        seed = args.seed if args.seed is not None else SEEDS.get(name, 7)
        prompt = PROMPTS.get(name, PROMPTS["base"]) + STYLE
        negative = NEGATIVES.get(name, NEGATIVE)
        result = paint(
            pipe, art, mask, name in LEFT, name in SMOOTH_START, prompt, negative, seed, args.steps
        )
        if name in LIGHTEN:
            white = Image.new("RGB", result.size, (255, 255, 255))
            result = Image.blend(result, white, LIGHTEN[name])
        result.save(wide / f"{name}.png")
        print(f"  → {wide / name}.png")


def cache_dir() -> Path:
    base = os.environ.get("XDG_CACHE_HOME") or Path.home() / ".cache"
    return Path(base) / "anstoss-3.1"


def export(sources: Path) -> None:
    """Has the app write the artwork of every dialog."""
    project = Path(__file__).resolve().parent.parent
    print("Exporting the dialog artwork …", flush=True)
    command = ["cargo", "run", "--release", "--quiet", "--", "--export-backgrounds"]
    subprocess.run(command, cwd=project, check=True)
    if not any(sources.glob("*_mask.png")):
        sys.exit(f"The app exported nothing to {sources}")


def load_pipeline():
    if not torch.cuda.is_available():
        sys.exit("No CUDA device; SDXL needs the GPU.")
    vae = AutoencoderKL.from_pretrained(VAE, torch_dtype=torch.float16)
    pipe = AutoPipelineForInpainting.from_pretrained(
        MODEL, vae=vae, torch_dtype=torch.float16, variant="fp16"
    )
    pipe.set_progress_bar_config(disable=True)
    return pipe.to("cuda")


def smooth_fill(image: Image.Image, mask: Image.Image) -> Image.Image:
    """Fills the masked pixels from the unmasked ones around them by repeated
    averaging, worked out on a smaller copy and blurred up."""
    small = (image.width // 8, image.height // 8)
    pixels = np.asarray(image.resize(small, Image.BOX), dtype=np.float32)
    hole = np.asarray(mask.resize(small, Image.BOX)) > 0
    known = pixels[~hole].mean(axis=0) if (~hole).any() else np.full(3, 200.0)
    pixels[hole] = known
    for _ in range(1500):
        padded = np.pad(pixels, ((1, 1), (1, 1), (0, 0)), mode="edge")
        average = (padded[:-2, 1:-1] + padded[2:, 1:-1] + padded[1:-1, :-2] + padded[1:-1, 2:]) / 4
        pixels[hole] = average[hole]
    fill = Image.fromarray(pixels.round().astype(np.uint8)).resize(image.size, Image.BICUBIC)
    fill = fill.filter(ImageFilter.GaussianBlur(6))
    return Image.composite(fill, image, mask)


def paint(
    pipe,
    art,
    mask,
    left: bool,
    smooth_start: bool,
    prompt: str,
    negative: str,
    seed: int,
    steps: int,
) -> Image.Image:
    """The picture on the canvas — at the left or in the middle — with what
    the mask marks, and all the canvas around the picture, repainted."""
    small = art.resize(GEN_ART, Image.LANCZOS)
    at = 0 if left else (GEN[0] - GEN_ART[0]) // 2

    # Start the new space from the mirrored edges, so the colours are right
    # from the first step; the model repaints it entirely.
    start = Image.new("RGB", GEN)
    start.paste(small, (at, 0))
    if at:
        start.paste(ImageOps.mirror(small.crop((0, 0, at, GEN[1]))), (0, 0))
    right = GEN[0] - at - GEN_ART[0]
    start.paste(ImageOps.mirror(small.crop((GEN_ART[0] - right, 0, GEN_ART[0], GEN[1]))), (at + GEN_ART[0], 0))

    inside = Image.new("L", GEN, 255 if smooth_start else 0)
    inside.paste(mask.resize(GEN_ART, Image.NEAREST), (at, 0))
    # What is repainted inside the picture starts out as a smooth fill from
    # its surroundings, so nothing of the old interface suggests new shapes.
    start = smooth_fill(start, inside.filter(ImageFilter.MaxFilter(5)))
    repaint = Image.new("L", GEN, 255)
    repaint.paste(mask.resize(GEN_ART, Image.NEAREST), (at, 0))
    # The new space reaches a little into the picture at its edges.
    edges = ImageDraw.Draw(repaint)
    if at:
        edges.rectangle((at, 0, at + OVERLAP, GEN[1]), fill=255)
    edges.rectangle((at + GEN_ART[0] - OVERLAP, 0, at + GEN_ART[0], GEN[1]), fill=255)
    repaint = repaint.filter(ImageFilter.MaxFilter(5)).filter(ImageFilter.GaussianBlur(OVERLAP / 3))

    painted = pipe(
        prompt=prompt,
        negative_prompt=negative,
        image=start,
        mask_image=repaint,
        width=GEN[0],
        height=GEN[1],
        strength=0.99,
        guidance_scale=7.0,
        num_inference_steps=steps,
        generator=torch.Generator("cuda").manual_seed(seed),
    ).images[0]

    # Scale so the picture covers ART exactly and crop to the canvas.
    scale = ART[0] / GEN_ART[0]
    big = painted.resize((round(GEN[0] * scale), CANVAS[1]), Image.LANCZOS)
    left_edge = 0 if left else (big.width - CANVAS[0]) // 2
    wide = big.crop((left_edge, 0, left_edge + CANVAS[0], CANVAS[1]))

    # The original goes back on top, untouched where nothing was repainted,
    # fading out over a soft seam towards what was.
    original = art.resize(ART, Image.LANCZOS)
    kept = ImageOps.invert(mask.resize(ART, Image.NEAREST).filter(ImageFilter.MaxFilter(9)))
    kept = np.asarray(kept.filter(ImageFilter.GaussianBlur(SEAM / 3)), dtype=np.float32)
    ramp = np.minimum(1.0, (np.arange(ART[0], dtype=np.float32) + 0.5) / SEAM)
    edges = ramp[::-1] if left else np.minimum(ramp, ramp[::-1])
    keep = Image.fromarray((kept * edges[np.newaxis, :]).round().astype(np.uint8))
    wide.paste(original, (0 if left else (CANVAS[0] - ART[0]) // 2, 0), keep)
    return wide


if __name__ == "__main__":
    main()
