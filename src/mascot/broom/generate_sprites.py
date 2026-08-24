"""
Pixel-Art Broom Sprite Sheet Generator for Broomed Mascot.
Generates 48x48 native pixel-art frames in horizontal sprite strips.
Produces 5 crisp 16-bit style sprite sheets:
- idle.png (6 frames)
- working.png (8 frames)
- success.png (8 frames)
- attention.png (6 frames)
- error.png (6 frames)
"""

import math
import os
from PIL import Image, ImageDraw

FRAME_W = 48
FRAME_H = 48

# ─── 16-bit Curated Color Palette ───
OUTLINE = (28, 22, 38, 255)         # Dark crisp border
HANDLE_SHADOW = (100, 55, 22, 255)  # Dark mahogany
HANDLE_MID = (165, 95, 40, 255)     # Warm wood
HANDLE_LIGHT = (215, 145, 75, 255)  # Wood highlight
HANDLE_KNOB = (235, 175, 105, 255)  # Top highlight

COLLAR_DARK = (130, 90, 20, 255)    # Dark brass
COLLAR_MID = (195, 145, 35, 255)    # Brass gold
COLLAR_LIGHT = (245, 205, 75, 255)  # Brass shine

FACE_BG = (35, 30, 48, 255)         # Dark cute face band
EYE_WHITE = (250, 250, 255, 255)
EYE_PUPIL = (28, 24, 40, 255)
EYE_GLINT = (120, 230, 255, 255)    # Tiny cyan highlight
CHEEK_BLUSH = (235, 110, 130, 210)  # Subtle blush

BRISTLE_DARK = (155, 110, 25, 255)  # Straw shadow
BRISTLE_MID = (210, 160, 45, 255)   # Straw body
BRISTLE_LIGHT = (245, 200, 75, 255) # Straw highlight
BRISTLE_TIP = (255, 228, 120, 255)  # Straw tips

SPARKLE = (255, 245, 140, 255)
SPARKLE_WHITE = (255, 255, 255, 255)
DUST = (175, 185, 205, 190)
SWEAT = (90, 200, 250, 240)


def put_pixel(img_data, w, h, x, y, color):
    """Safely place a pixel on the canvas."""
    ix, iy = int(round(x)), int(round(y))
    if 0 <= ix < w and 0 <= iy < h:
        img_data[ix, iy] = color


def draw_pixel_box(img_data, w, h, x1, y1, x2, y2, fill, outline=None):
    """Draw a filled pixel box with optional outline."""
    min_x, max_x = min(x1, x2), max(x1, x2)
    min_y, max_y = min(y1, y2), max(y1, y2)
    for y in range(min_y, max_y + 1):
        for x in range(min_x, max_x + 1):
            is_edge = (x == min_x or x == max_x or y == min_y or y == max_y)
            if is_edge and outline:
                put_pixel(img_data, w, h, x, y, outline)
            else:
                put_pixel(img_data, w, h, x, y, fill)


def render_broom_sprite(
    img_data,
    offset_x=0,
    cx=24,
    base_y=28,
    angle=0.0,
    squash_y=0,
    eye_style="normal",
    blush=True,
    show_sparkles=False,
    sparkle_frame=0,
    dust_dir=0,
    sweat_drop=False,
    exclamation=False,
):
    """
    Renders a pixel-perfect broom character with rotation, compression,
    and facial expressions at native 48x48.
    """
    rad = math.radians(angle)
    cos_a = math.cos(rad)
    sin_a = math.sin(rad)

    def transform(lx, ly):
        """Transform local broom coordinates around pivot (cx, base_y) to canvas coords."""
        rx = lx * cos_a - ly * sin_a
        ry = lx * sin_a + ly * cos_a
        return offset_x + cx + rx, base_y + ry

    # ── 1. Bristles (Layer 1: base Straw bundle) ──
    # The bristles fan out slightly downwards
    # Width: 18px at base, height: 13px - squash_y
    bristle_h = max(8, 14 - squash_y)
    bristle_top_w = 12
    bristle_bot_w = 18

    # Outline + Fill of Bristles
    for by in range(0, bristle_h):
        progress = by / float(bristle_h - 1) if bristle_h > 1 else 0
        w_curr = int(round(bristle_top_w + (bristle_bot_w - bristle_top_w) * progress))
        half_w = w_curr // 2

        for bx in range(-half_w, half_w + 1):
            is_outer = (bx == -half_w or bx == half_w or by == bristle_h - 1)
            tx, ty = transform(bx, by + 5)

            if is_outer:
                put_pixel(img_data, FRAME_W * 8, FRAME_H, tx, ty, OUTLINE)
            else:
                # Straw texture: vertical striping & highlights
                if by == bristle_h - 2:
                    c = BRISTLE_TIP
                elif (bx + by) % 3 == 0:
                    c = BRISTLE_LIGHT
                elif bx < -1:
                    c = BRISTLE_MID
                elif bx > 2:
                    c = BRISTLE_DARK
                else:
                    c = BRISTLE_MID
                put_pixel(img_data, FRAME_W * 8, FRAME_H, tx, ty, c)

    # Jagged bottom bristle tips
    for bx in range(-bristle_bot_w // 2 + 1, bristle_bot_w // 2):
        if (bx * 7) % 3 == 0:
            tx, ty = transform(bx, bristle_h + 5)
            put_pixel(img_data, FRAME_W * 8, FRAME_H, tx, ty, OUTLINE)
            tx_in, ty_in = transform(bx, bristle_h + 4)
            put_pixel(img_data, FRAME_W * 8, FRAME_H, tx_in, ty_in, BRISTLE_TIP)

    # ── 2. Ferrule / Collar (Layer 2: brass connector with Face Screen) ──
    collar_h = 7
    collar_w = 12
    c_half = collar_w // 2

    # Outline
    for cy in range(-collar_h, 1):
        for cx_l in range(-c_half, c_half + 1):
            is_edge = (cx_l == -c_half or cx_l == c_half or cy == -collar_h or cy == 0)
            tx, ty = transform(cx_l, cy + 4)
            if is_edge:
                put_pixel(img_data, FRAME_W * 8, FRAME_H, tx, ty, OUTLINE)
            else:
                # Inner Face Screen / Brass
                if -collar_h + 1 <= cy <= -1 and -c_half + 1 <= cx_l <= c_half - 1:
                    put_pixel(img_data, FRAME_W * 8, FRAME_H, tx, ty, FACE_BG)
                else:
                    put_pixel(img_data, FRAME_W * 8, FRAME_H, tx, ty, COLLAR_MID)

    # Brass highlights top & bottom borders
    for cx_l in range(-c_half + 1, c_half):
        tx, ty = transform(cx_l, -collar_h + 1 + 4)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, tx, ty, COLLAR_LIGHT)

    # ── 3. Face Details on the Ferrule ──
    # Face center is around cy = -3
    eye_y = -3 + 4
    left_eye_x = -3
    right_eye_x = 2

    if eye_style == "normal":
        # Cute pixel eyes (2x2 with glint)
        for ex in (left_eye_x, right_eye_x):
            tx, ty = transform(ex, eye_y)
            put_pixel(img_data, FRAME_W * 8, FRAME_H, tx, ty, EYE_WHITE)
            tx1, ty1 = transform(ex + 1, eye_y)
            put_pixel(img_data, FRAME_W * 8, FRAME_H, tx1, ty1, EYE_PUPIL)
            tx2, ty2 = transform(ex, eye_y + 1)
            put_pixel(img_data, FRAME_W * 8, FRAME_H, tx2, ty2, EYE_GLINT)
            tx3, ty3 = transform(ex + 1, eye_y + 1)
            put_pixel(img_data, FRAME_W * 8, FRAME_H, tx3, ty3, EYE_PUPIL)

        # Tiny cute smile
        tx_m, ty_m = transform(0, eye_y + 1)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, tx_m, ty_m, EYE_WHITE)

    elif eye_style == "blink":
        # Sleeping/blinking line eyes (- -)
        for ex in (left_eye_x, right_eye_x):
            tx, ty = transform(ex, eye_y + 1)
            put_pixel(img_data, FRAME_W * 8, FRAME_H, tx, ty, EYE_WHITE)
            tx1, ty1 = transform(ex + 1, eye_y + 1)
            put_pixel(img_data, FRAME_W * 8, FRAME_H, tx1, ty1, EYE_WHITE)

    elif eye_style == "happy":
        # Happy arched eyes (^ ^)
        for ex in (left_eye_x, right_eye_x):
            tx, ty = transform(ex, eye_y)
            put_pixel(img_data, FRAME_W * 8, FRAME_H, tx, ty, EYE_WHITE)
            tx1, ty1 = transform(ex + 1, eye_y)
            put_pixel(img_data, FRAME_W * 8, FRAME_H, tx1, ty1, EYE_WHITE)
            tx2, ty2 = transform(ex - 1, eye_y + 1)
            put_pixel(img_data, FRAME_W * 8, FRAME_H, tx2, ty2, EYE_WHITE)
            tx3, ty3 = transform(ex + 2, eye_y + 1)
            put_pixel(img_data, FRAME_W * 8, FRAME_H, tx3, ty3, EYE_WHITE)

        # Open happy mouth
        tx_m, ty_m = transform(0, eye_y + 1)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, tx_m, ty_m, (240, 120, 130, 255))

    elif eye_style == "focused":
        # Determined / focused sweeping eyes (> <)
        # Left eye: >
        tx, ty = transform(left_eye_x, eye_y)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, tx, ty, EYE_WHITE)
        tx1, ty1 = transform(left_eye_x + 1, eye_y + 1)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, tx1, ty1, EYE_WHITE)
        tx2, ty2 = transform(left_eye_x, eye_y + 2)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, tx2, ty2, EYE_WHITE)
        # Right eye: <
        tx3, ty3 = transform(right_eye_x + 1, eye_y)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, tx3, ty3, EYE_WHITE)
        tx4, ty4 = transform(right_eye_x, eye_y + 1)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, tx4, ty4, EYE_WHITE)
        tx5, ty5 = transform(right_eye_x + 1, eye_y + 2)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, tx5, ty5, EYE_WHITE)

    elif eye_style == "worried":
        # Startled / error eyes (o _ O)
        tx, ty = transform(left_eye_x, eye_y)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, tx, ty, EYE_WHITE)
        tx1, ty1 = transform(left_eye_x, eye_y + 1)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, tx1, ty1, EYE_WHITE)
        tx2, ty2 = transform(right_eye_x, eye_y - 1)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, tx2, ty2, EYE_WHITE)
        tx3, ty3 = transform(right_eye_x + 1, eye_y)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, tx3, ty3, EYE_WHITE)
        tx4, ty4 = transform(right_eye_x, eye_y + 1)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, tx4, ty4, EYE_WHITE)
        # Wavy worried mouth
        tx_m, ty_m = transform(0, eye_y + 2)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, tx_m, ty_m, EYE_WHITE)

    elif eye_style == "curious":
        # Wide looking up eyes (attention state)
        for ex in (left_eye_x, right_eye_x):
            tx, ty = transform(ex, eye_y - 1)
            put_pixel(img_data, FRAME_W * 8, FRAME_H, tx, ty, EYE_WHITE)
            tx1, ty1 = transform(ex + 1, eye_y - 1)
            put_pixel(img_data, FRAME_W * 8, FRAME_H, tx1, ty1, EYE_GLINT)
            tx2, ty2 = transform(ex, eye_y)
            put_pixel(img_data, FRAME_W * 8, FRAME_H, tx2, ty2, EYE_PUPIL)
            tx3, ty3 = transform(ex + 1, eye_y)
            put_pixel(img_data, FRAME_W * 8, FRAME_H, tx3, ty3, EYE_PUPIL)
        # Small 'o' mouth
        tx_m, ty_m = transform(0, eye_y + 1)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, tx_m, ty_m, EYE_WHITE)

    if blush:
        # Subtle cheeks
        tx_b1, ty_b1 = transform(left_eye_x - 1, eye_y + 1)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, tx_b1, ty_b1, CHEEK_BLUSH)
        tx_b2, ty_b2 = transform(right_eye_x + 2, eye_y + 1)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, tx_b2, ty_b2, CHEEK_BLUSH)

    # ── 4. Handle (Layer 3: Wooden Pole & Knob) ──
    handle_len = 22
    for hy in range(-handle_len, -collar_h + 1):
        for hx in (-2, -1, 0, 1):
            tx, ty = transform(hx, hy + 4)
            if hx == -2 or hx == 1:
                put_pixel(img_data, FRAME_W * 8, FRAME_H, tx, ty, OUTLINE)
            elif hx == -1:
                put_pixel(img_data, FRAME_W * 8, FRAME_H, tx, ty, HANDLE_LIGHT)
            else:
                put_pixel(img_data, FRAME_W * 8, FRAME_H, tx, ty, HANDLE_MID)

    # Top Knob / Cap
    knob_y = -handle_len - 1
    for ky in (knob_y - 2, knob_y - 1, knob_y):
        for kx in (-3, -2, -1, 0, 1, 2):
            is_edge = (kx == -3 or kx == 2 or ky == knob_y - 2 or ky == knob_y)
            tx, ty = transform(kx, ky + 4)
            if is_edge:
                put_pixel(img_data, FRAME_W * 8, FRAME_H, tx, ty, OUTLINE)
            elif kx == -1 or ky == knob_y - 1:
                put_pixel(img_data, FRAME_W * 8, FRAME_H, tx, ty, HANDLE_KNOB)
            else:
                put_pixel(img_data, FRAME_W * 8, FRAME_H, tx, ty, HANDLE_LIGHT)

    # ── 5. Effects & Particles (Dust, Sparkles, Sweat, Exclamation) ──
    if dust_dir != 0:
        # Dust puffs at the base
        base_x = offset_x + cx + (12 if dust_dir > 0 else -12)
        base_y_floor = base_y + 17
        # Puff 1
        put_pixel(img_data, FRAME_W * 8, FRAME_H, base_x, base_y_floor, DUST)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, base_x + dust_dir * 2, base_y_floor - 1, DUST)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, base_x + dust_dir * 4, base_y_floor, DUST)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, base_x + dust_dir * 1, base_y_floor - 2, (220, 225, 240, 160))
        # Sweeping speed lines
        put_pixel(img_data, FRAME_W * 8, FRAME_H, base_x - dust_dir * 4, base_y_floor, (230, 235, 245, 120))
        put_pixel(img_data, FRAME_W * 8, FRAME_H, base_x - dust_dir * 8, base_y_floor, (230, 235, 245, 80))

    if show_sparkles:
        # Little 4-point sparkle stars
        sparkles = [
            (offset_x + cx + 14, base_y - 12 + sparkle_frame),
            (offset_x + cx - 14, base_y - 8 - sparkle_frame),
            (offset_x + cx + 11, base_y + 4),
        ]
        for sx, sy in sparkles:
            put_pixel(img_data, FRAME_W * 8, FRAME_H, sx, sy, SPARKLE_WHITE)
            put_pixel(img_data, FRAME_W * 8, FRAME_H, sx + 1, sy, SPARKLE)
            put_pixel(img_data, FRAME_W * 8, FRAME_H, sx - 1, sy, SPARKLE)
            put_pixel(img_data, FRAME_W * 8, FRAME_H, sx, sy + 1, SPARKLE)
            put_pixel(img_data, FRAME_W * 8, FRAME_H, sx, sy - 1, SPARKLE)

    if sweat_drop:
        # Little blue sweat drop on right side
        sx = offset_x + cx + 10
        sy = base_y - 12
        put_pixel(img_data, FRAME_W * 8, FRAME_H, sx, sy, SWEAT)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, sx, sy + 1, SWEAT)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, sx + 1, sy + 1, SWEAT)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, sx, sy + 2, (200, 240, 255, 255))

    if exclamation:
        # Inquisitive '!' mark above broom
        ex_x = offset_x + cx + 10
        ex_y = base_y - 24
        put_pixel(img_data, FRAME_W * 8, FRAME_H, ex_x, ex_y, (255, 215, 60, 255))
        put_pixel(img_data, FRAME_W * 8, FRAME_H, ex_x, ex_y + 1, (255, 215, 60, 255))
        put_pixel(img_data, FRAME_W * 8, FRAME_H, ex_x, ex_y + 2, (255, 215, 60, 255))
        put_pixel(img_data, FRAME_W * 8, FRAME_H, ex_x, ex_y + 4, (255, 215, 60, 255))
        # Outline for '!'
        put_pixel(img_data, FRAME_W * 8, FRAME_H, ex_x - 1, ex_y, OUTLINE)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, ex_x + 1, ex_y, OUTLINE)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, ex_x - 1, ex_y + 4, OUTLINE)
        put_pixel(img_data, FRAME_W * 8, FRAME_H, ex_x + 1, ex_y + 4, OUTLINE)


# ─── Sprite Sheet Builders ───

def build_idle_sheet():
    """6 frames: breathing 1px bob, blink on frames 3-4, subtle tip sway."""
    num_frames = 6
    img = Image.new("RGBA", (FRAME_W * num_frames, FRAME_H), (0, 0, 0, 0))
    data = img.load()

    bobs = [0, 1, 1, 0, -1, 0]
    angles = [0.0, 1.0, 0.5, 0.0, -1.0, -0.5]
    eyes = ["normal", "normal", "blink", "blink", "normal", "normal"]

    for i in range(num_frames):
        render_broom_sprite(
            data,
            offset_x=i * FRAME_W,
            cx=24,
            base_y=28 + bobs[i],
            angle=angles[i],
            squash_y=bobs[i] if bobs[i] > 0 else 0,
            eye_style=eyes[i],
            blush=True,
        )
    return img


def build_working_sheet():
    """8 frames: full sweeping cycle with tilt, bristle compression & dust puffs."""
    num_frames = 8
    img = Image.new("RGBA", (FRAME_W * num_frames, FRAME_H), (0, 0, 0, 0))
    data = img.load()

    # Dynamic sweeping motion: lean left -> sweep across right -> return
    angles = [-14.0, -8.0, 4.0, 15.0, 12.0, 5.0, -4.0, -10.0]
    cxs = [20, 22, 25, 28, 27, 25, 23, 21]
    squashes = [2, 1, 2, 3, 2, 1, 0, 1]
    dusts = [0, 1, 1, 1, -1, -1, 0, 0]

    for i in range(num_frames):
        render_broom_sprite(
            data,
            offset_x=i * FRAME_W,
            cx=cxs[i],
            base_y=27,
            angle=angles[i],
            squash_y=squashes[i],
            eye_style="focused",
            blush=True,
            dust_dir=dusts[i],
        )
    return img


def build_success_sheet():
    """8 frames: celebratory hop, happy ^ ^ eyes, sparkling burst."""
    num_frames = 8
    img = Image.new("RGBA", (FRAME_W * num_frames, FRAME_H), (0, 0, 0, 0))
    data = img.load()

    # Y offsets for vertical hop: crouch -> jump -> apex -> float -> land -> settle
    y_offsets = [1, -3, -6, -7, -5, -2, 1, 0]
    squashes = [2, 0, 0, 0, 0, 0, 2, 0]
    sparkles = [False, False, True, True, True, False, False, False]
    angles = [0.0, -3.0, -5.0, 0.0, 5.0, 2.0, 0.0, 0.0]

    for i in range(num_frames):
        render_broom_sprite(
            data,
            offset_x=i * FRAME_W,
            cx=24,
            base_y=28 + y_offsets[i],
            angle=angles[i],
            squash_y=squashes[i],
            eye_style="happy",
            blush=True,
            show_sparkles=sparkles[i],
            sparkle_frame=i % 3,
        )
    return img


def build_attention_sheet():
    """6 frames: curious head-tilt toward user, wide eyes, '!' prompt."""
    num_frames = 6
    img = Image.new("RGBA", (FRAME_W * num_frames, FRAME_H), (0, 0, 0, 0))
    data = img.load()

    angles = [0.0, 6.0, 9.0, 9.0, 6.0, 2.0]
    bobs = [0, -1, -2, -2, -1, 0]
    exclamations = [False, True, True, True, False, False]

    for i in range(num_frames):
        render_broom_sprite(
            data,
            offset_x=i * FRAME_W,
            cx=24,
            base_y=28 + bobs[i],
            angle=angles[i],
            squash_y=0,
            eye_style="curious",
            blush=True,
            exclamation=exclamations[i],
        )
    return img


def build_error_sheet():
    """6 frames: startled recoil, shudder shake, worried face, sweat drop."""
    num_frames = 6
    img = Image.new("RGBA", (FRAME_W * num_frames, FRAME_H), (0, 0, 0, 0))
    data = img.load()

    # Recoil & horizontal shudder
    cxs = [27, 21, 26, 22, 25, 24]
    angles = [8.0, -6.0, 5.0, -4.0, 2.0, 0.0]
    sweat = [True, True, True, True, False, False]

    for i in range(num_frames):
        render_broom_sprite(
            data,
            offset_x=i * FRAME_W,
            cx=cxs[i],
            base_y=28,
            angle=angles[i],
            squash_y=0,
            eye_style="worried",
            blush=False,
            sweat_drop=sweat[i],
        )
    return img


def main():
    out_dir = os.path.dirname(os.path.abspath(__file__))

    sheets = {
        "idle": (build_idle_sheet(), 6),
        "working": (build_working_sheet(), 8),
        "success": (build_success_sheet(), 8),
        "attention": (build_attention_sheet(), 6),
        "error": (build_error_sheet(), 6),
    }

    for state, (img, frames) in sheets.items():
        png_path = os.path.join(out_dir, f"{state}.png")
        webp_path = os.path.join(out_dir, f"{state}.webp")
        img.save(png_path, "PNG")
        img.save(webp_path, "WEBP", lossless=True)
        print(f"Generated {state}: {img.width}x{img.height} ({frames} frames)")


if __name__ == "__main__":
    main()
