"""
8-Frame Animation Generator for Broomed Mascot Assets.
Generates 8 high-resolution frames, WebP sequences, animated GIFs, and spritesheets for:
1. broomed_reference.png -> mascot/reference/ (Idle & Breathe / Blink)
2. broomed-ai.png        -> mascot/ai/        (Neural Pulse & Hologram)
3. broomed-celebrate.png -> mascot/celebrate/ (Victory Jump & Confetti)
4. broomed-clean.png     -> mascot/clean/     (Polish Shine & Bubbles)
5. broomed-fast.png      -> mascot/fast/      (Sonic Dash & Speed Trails)
6. broomed-reading.png   -> mascot/reading/   (Page Turn & Study)
7. broomed-search.png    -> mascot/search/    (Magnifying Glass & Clue Scan)
8. broomed-shield.png    -> mascot/shield/    (Cyber Barrier & Defense Pulse)
9. broomed-sleeping.png  -> mascot/sleeping/  (Snore Breathing & Floating Zzz)
"""

import os
import math
import random
from PIL import Image, ImageDraw, ImageFilter, ImageEnhance, ImageOps

BASE_DIR = r"c:\Projects\broomed\src\assets"
MASCOT_DIR = os.path.join(BASE_DIR, "mascot")

# Ensure mascot base directory exists
os.makedirs(MASCOT_DIR, exist_ok=True)


# ─── Helper Functions ───────────────────────────────────────────

def transform_layer(img, dx=0, dy=0, angle=0.0, scale_x=1.0, scale_y=1.0, pivot=None):
    """
    Transforms an RGBA image with scaling, rotation around pivot, and translation,
    retaining full canvas bounds and crisp transparency.
    """
    w, h = img.size
    if pivot is None:
        pivot = (w / 2.0, h / 2.0)
    
    # Scale first if needed
    if abs(scale_x - 1.0) > 0.001 or abs(scale_y - 1.0) > 0.001:
        new_w = max(1, int(round(w * scale_x)))
        new_h = max(1, int(round(h * scale_y)))
        scaled = img.resize((new_w, new_h), Image.Resampling.BILINEAR)
        
        # Place scaled back onto a canvas centered relative to pivot
        canvas = Image.new("RGBA", (w, h), (0, 0, 0, 0))
        # calculate paste offset to keep pivot aligned
        px, py = pivot
        offset_x = int(round(px - px * scale_x))
        offset_y = int(round(py - py * scale_y))
        canvas.paste(scaled, (offset_x, offset_y), scaled)
        current = canvas
    else:
        current = img.copy()

    # Rotate around pivot if needed
    if abs(angle) > 0.01:
        # Create an expanded workspace to avoid clipping during rotation
        exp_w, exp_h = w * 2, h * 2
        exp = Image.new("RGBA", (exp_w, exp_h), (0, 0, 0, 0))
        exp_pivot_x = pivot[0] + (exp_w - w) / 2.0
        exp_pivot_y = pivot[1] + (exp_h - h) / 2.0
        
        exp.paste(current, (int((exp_w - w) / 2), int((exp_h - h) / 2)), current)
        rotated = exp.rotate(angle, resample=Image.Resampling.BILINEAR, center=(exp_pivot_x, exp_pivot_y))
        
        # Crop back to original size
        crop_box = (
            int((exp_w - w) / 2),
            int((exp_h - h) / 2),
            int((exp_w - w) / 2 + w),
            int((exp_h - h) / 2 + h)
        )
        current = rotated.crop(crop_box)

    # Translate if needed
    if abs(dx) > 0.01 or abs(dy) > 0.01:
        translated = Image.new("RGBA", (w, h), (0, 0, 0, 0))
        translated.paste(current, (int(round(dx)), int(round(dy))), current)
        current = translated

    return current


def draw_star_sparkle(draw, cx, cy, size, color, core_color=(255, 255, 255, 255), angle=0.0):
    """Draws a crisp 4-point shining diamond star with central core glow."""
    rad = math.radians(angle)
    cos_a, sin_a = math.cos(rad), math.sin(rad)
    
    def rot_pt(x, y):
        rx = x * cos_a - y * sin_a
        ry = x * sin_a + y * cos_a
        return (cx + rx, cy + ry)

    s = size
    w = max(1.5, size * 0.22)
    # 4 points: top, bottom, left, right
    poly = [
        rot_pt(0, -s),
        rot_pt(w, -w),
        rot_pt(s, 0),
        rot_pt(w, w),
        rot_pt(0, s),
        rot_pt(-w, w),
        rot_pt(-s, 0),
        rot_pt(-w, -w)
    ]
    draw.polygon(poly, fill=color)
    
    # Core bright flare
    core_s = s * 0.4
    core_w = core_s * 0.25
    core_poly = [
        rot_pt(0, -core_s),
        rot_pt(core_w, -core_w),
        rot_pt(core_s, 0),
        rot_pt(core_w, core_w),
        rot_pt(0, core_s),
        rot_pt(-core_w, core_w),
        rot_pt(-core_s, 0),
        rot_pt(-core_w, -core_w)
    ]
    draw.polygon(core_poly, fill=core_color)


def draw_bubble(draw, cx, cy, r, alpha=200):
    """Draws an iridescent translucent soap bubble with highlight crescent."""
    if r <= 2:
        return
    # Body
    bbox = (cx - r, cy - r, cx + r, cy + r)
    draw.ellipse(bbox, fill=(210, 240, 255, int(alpha * 0.22)), outline=(220, 245, 255, int(alpha * 0.85)), width=max(1, int(r * 0.08)))
    # Highlight crescent arc / glint
    hr = r * 0.65
    hx = cx - r * 0.35
    hy = cy - r * 0.35
    draw.arc((hx - hr, hy - hr, hx + hr, hy + hr), start=200, end=310, fill=(255, 255, 255, int(alpha * 0.95)), width=max(1, int(r * 0.14)))
    # Secondary bottom refraction
    draw.arc((cx - r*0.7, cy - r*0.7, cx + r*0.7, cy + r*0.7), start=40, end=90, fill=(255, 210, 245, int(alpha * 0.5)), width=max(1, int(r * 0.08)))


def draw_confetti_piece(draw, cx, cy, w, h, angle_deg, color):
    """Draws a fluttering rectangular confetti flake with 3D spin compression."""
    rad = math.radians(angle_deg)
    cos_a, sin_a = math.cos(rad), math.sin(rad)
    
    hw = w / 2.0
    hh = h / 2.0
    corners = [
        (-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)
    ]
    poly = []
    for lx, ly in corners:
        rx = lx * cos_a - ly * sin_a
        ry = lx * sin_a + ly * cos_a
        poly.append((cx + rx, cy + ry))
    
    draw.polygon(poly, fill=color)


def draw_letter_z(draw, x, y, size, color, width=None):
    """Draws a stylized, clean 'Z' letter for sleeping animations."""
    if width is None:
        width = max(2, int(size * 0.2))
    
    w = size * 0.75
    h = size
    
    # Top bar
    draw.line([(x, y), (x + w, y)], fill=color, width=width)
    # Diagonal
    draw.line([(x + w, y), (x, y + h)], fill=color, width=width)
    # Bottom bar
    draw.line([(x, y + h), (x + w, y + h)], fill=color, width=width)


def save_animation_pack(frames, folder_name, base_filename):
    """
    Saves an 8-frame animation suite:
    - 8 PNG frames: broomed-<name>-1.png .. 8.png
    - 8 WebP frames: broomed-<name>-1.webp .. 8.webp
    - Animated GIF: broomed-<name>.gif (seamless loop at 140ms/frame)
    - Spritesheet: spritesheet.png and spritesheet.webp (horizontal strip)
    """
    out_dir = os.path.join(MASCOT_DIR, folder_name)
    os.makedirs(out_dir, exist_ok=True)
    
    w, h = frames[0].size
    num_frames = len(frames)
    
    # 1. Save individual PNG and WebP frames
    for i, frame in enumerate(frames):
        idx = i + 1
        png_path = os.path.join(out_dir, f"{base_filename}-{idx}.png")
        webp_path = os.path.join(out_dir, f"{base_filename}-{idx}.webp")
        frame.save(png_path, format="PNG", optimize=True)
        frame.save(webp_path, format="WEBP", lossless=True)
        
    # 2. Save Animated GIF
    gif_path = os.path.join(out_dir, f"{base_filename}.gif")
    gif_frames = []
    for f in frames:
        # Convert RGBA to GIF with transparency preserved
        alpha = f.split()[-1]
        mask = Image.eval(alpha, lambda a: 255 if a > 30 else 0)
        # Create solid canvas with transparent color key
        bg = Image.new("RGBA", (w, h), (0, 0, 0, 0))
        bg.paste(f, mask=mask)
        gif_frames.append(bg.convert("RGBA"))
        
    gif_frames[0].save(
        gif_path,
        save_all=True,
        append_images=gif_frames[1:],
        duration=150,
        loop=0,
        disposal=2
    )
    
    # 3. Save Spritesheet (horizontal strip)
    sheet_w = w * num_frames
    sheet = Image.new("RGBA", (sheet_w, h), (0, 0, 0, 0))
    for i, f in enumerate(frames):
        sheet.paste(f, (i * w, 0), f)
        
    sheet.save(os.path.join(out_dir, "spritesheet.png"), format="PNG", optimize=True)
    sheet.save(os.path.join(out_dir, "spritesheet.webp"), format="WEBP", lossless=True)
    
    print(f"[OK] [{folder_name}] Successfully created 8 frames + GIF + Spritesheets in: {out_dir}")


# ─── 1. REFERENCE / IDLE ANIMATION ──────────────────────────────
def generate_reference_animation():
    src_path = os.path.join(BASE_DIR, "broomed_reference.png")
    base_img = Image.open(src_path).convert("RGBA")
    w, h = base_img.size
    cx, cy = w / 2.0, h / 2.0
    pivot = (cx, h * 0.88)
    
    frames = []
    # 8-frame natural breathing + blinking cycle
    for f in range(8):
        t = f / 8.0 * 2.0 * math.pi
        
        # Breathing vertical bob & squash-stretch
        # Squeeze down on frames 2-3, rise up on frames 5-6
        dy = -5.0 * math.sin(t)
        scale_y = 1.0 + 0.025 * math.sin(t)
        scale_x = 1.0 / math.sqrt(scale_y)
        angle = 1.2 * math.sin(t)
        
        frame = transform_layer(base_img, dx=0, dy=dy, angle=angle, scale_x=scale_x, scale_y=scale_y, pivot=pivot)
        draw = ImageDraw.Draw(frame)
        
        # Eye region: ~ x: 235..275, y: 140..175 (scaled relative to image size)
        eye_cx = int(w * 0.49)
        eye_cy = int(h * 0.245 + dy)
        eye_w = int(w * 0.08)
        eye_h = int(h * 0.055)
        
        # Blinking on frames 2 (half), 3 (full blink), 4 (half open)
        if f == 2 or f == 4:
            # Half-closed eyelid
            draw.ellipse((eye_cx - eye_w, eye_cy - eye_h*0.3, eye_cx + eye_w, eye_cy + eye_h*0.5), fill=(28, 22, 38, 255))
        elif f == 3:
            # Full blink happy curve (⌒)
            draw.arc((eye_cx - eye_w, eye_cy - eye_h*0.4, eye_cx + eye_w, eye_cy + eye_h*0.6), start=20, end=160, fill=(28, 22, 38, 255), width=4)
            # Cute blush
            draw.ellipse((eye_cx - eye_w*1.5, eye_cy + eye_h*0.5, eye_cx - eye_w*0.8, eye_cy + eye_h*0.9), fill=(250, 165, 165, 180))
            draw.ellipse((eye_cx + eye_w*0.8, eye_cy + eye_h*0.5, eye_cx + eye_w*1.5, eye_cy + eye_h*0.9), fill=(250, 165, 165, 180))
            
        # Subtle gentle handle shine on frame 5-6
        if f == 5 or f == 6:
            glint_y = int(h * 0.12 + dy)
            draw_star_sparkle(draw, int(w * 0.52), glint_y, size=10 if f == 5 else 6, color=(255, 235, 150, 220))
            
        frames.append(frame)
        
    save_animation_pack(frames, "reference", "broomed-reference")


# ─── 2. AI / NEURAL HOLOGRAM ANIMATION ──────────────────────────
def generate_ai_animation():
    src_path = os.path.join(BASE_DIR, "broomed-ai.png")
    base_img = Image.open(src_path).convert("RGBA")
    w, h = base_img.size
    cx, cy = w / 2.0, h / 2.0
    pivot = (cx, h * 0.85)
    
    frames = []
    # 8-frame levitation hum + neural pulse wave
    for f in range(8):
        t = f / 8.0 * 2.0 * math.pi
        
        # Smooth hovering levitation
        dy = -8.0 * math.sin(t)
        scale_y = 1.0 + 0.015 * math.sin(t * 2)
        scale_x = 1.0 / math.sqrt(scale_y)
        angle = 0.8 * math.cos(t)
        
        frame = transform_layer(base_img, dx=0, dy=dy, angle=angle, scale_x=scale_x, scale_y=scale_y, pivot=pivot)
        draw = ImageDraw.Draw(frame)
        
        # Neural nodes and circuit energy rings
        core_x = int(w * 0.5)
        core_y = int(h * 0.28 + dy)
        
        # Pulsing holographic orbit rings in 3D perspective
        pulse_phase = (f / 8.0)
        ring_r1 = int(w * 0.32 + 15 * math.sin(t))
        ring_r2 = int(h * 0.10 + 5 * math.sin(t))
        
        # Back half of orbit ring
        ring_color = (56, 189, 248, int(160 + 70 * math.sin(t)))
        draw.arc((core_x - ring_r1, core_y - ring_r2, core_x + ring_r1, core_y + ring_r2), start=180, end=360, fill=ring_color, width=3)
        
        # Floating data bits / neural sparks
        for bit_i in range(6):
            bit_angle = t + bit_i * (math.pi / 3.0)
            bx = core_x + int(ring_r1 * math.cos(bit_angle))
            by = core_y + int(ring_r2 * math.sin(bit_angle))
            bit_size = 4 + 2 * math.sin(bit_angle)
            bit_col = (125, 211, 252, 240) if bit_i % 2 == 0 else (251, 191, 36, 230)
            draw.rectangle((bx - bit_size, by - bit_size, bx + bit_size, by + bit_size), fill=bit_col)
            
        # Front half of orbit ring
        draw.arc((core_x - ring_r1, core_y - ring_r2, core_x + ring_r1, core_y + ring_r2), start=0, end=180, fill=ring_color, width=3)
        
        # Forehead AI Spark / Eureka Node
        node_intensity = max(0.0, math.sin(t + math.pi/4))
        if node_intensity > 0.3:
            spark_size = int(14 * node_intensity)
            draw_star_sparkle(draw, core_x, core_y - int(h*0.08), size=spark_size, color=(0, 240, 255, 230), angle=f * 22.5)
            
        # Digital scanning eye beam on frame 3 & 4
        if f in (3, 4):
            scan_y = core_y - int(h * 0.03) + (f - 3) * 8
            draw.line([(core_x - int(w*0.12), scan_y), (core_x + int(w*0.12), scan_y)], fill=(56, 189, 248, 200), width=3)
            
        frames.append(frame)
        
    save_animation_pack(frames, "ai", "broomed-ai")


# ─── 3. CELEBRATE / VICTORY JUMP ANIMATION ──────────────────────
def generate_celebrate_animation():
    src_path = os.path.join(BASE_DIR, "broomed-celebrate.png")
    base_img = Image.open(src_path).convert("RGBA")
    w, h = base_img.size
    cx, cy = w / 2.0, h / 2.0
    pivot = (cx, h * 0.9)
    
    frames = []
    
    # Pre-generate deterministic confetti particles across 8 frames
    random.seed(42)
    confetti_pool = []
    colors = [
        (239, 68, 68, 255),   # Red
        (245, 158, 11, 255),  # Gold/Amber
        (16, 185, 129, 255),  # Green
        (59, 130, 246, 255),  # Blue
        (236, 72, 153, 255),  # Pink
        (168, 85, 247, 255),  # Purple
        (254, 240, 138, 255)  # Bright yellow
    ]
    for _ in range(40):
        confetti_pool.append({
            "x0": random.uniform(w * 0.15, w * 0.85),
            "y0": random.uniform(h * 0.1, h * 0.5),
            "vx": random.uniform(-40, 40),
            "vy": random.uniform(-60, 20),
            "color": random.choice(colors),
            "w": random.uniform(8, 16),
            "h": random.uniform(5, 10),
            "spin": random.uniform(30, 90)
        })
    
    jump_offsets = [0, -18, -42, -38, -20, 6, -6, 0]
    squash_scales = [0.95, 1.08, 1.04, 1.01, 0.98, 0.92, 1.03, 1.00]
    angles = [0.0, -2.0, 3.0, -2.5, 1.5, -1.0, 0.5, 0.0]
    
    for f in range(8):
        dy = jump_offsets[f]
        sy = squash_scales[f]
        sx = 1.0 / math.sqrt(sy)
        angle = angles[f]
        
        frame = transform_layer(base_img, dx=0, dy=dy, angle=angle, scale_x=sx, scale_y=sy, pivot=pivot)
        draw = ImageDraw.Draw(frame)
        
        # Render dynamic confetti particles
        for p in confetti_pool:
            progress = (f / 8.0)
            # Parabolic trajectory
            px = p["x0"] + p["vx"] * progress
            py = p["y0"] + p["vy"] * progress + 80 * (progress ** 2)
            cur_angle = f * p["spin"]
            
            # Spin effect alters perceived width
            perceived_w = p["w"] * abs(math.cos(math.radians(cur_angle)))
            draw_confetti_piece(draw, px, py, max(2, perceived_w), p["h"], cur_angle, p["color"])
            
        # Celebration star bursts around apex (frames 2, 3, 4)
        if f in (2, 3, 4):
            draw_star_sparkle(draw, int(w * 0.2), int(h * 0.25 + dy), size=14, color=(251, 191, 36, 230), angle=f*30)
            draw_star_sparkle(draw, int(w * 0.8), int(h * 0.28 + dy), size=12, color=(56, 189, 248, 230), angle=-f*30)
            draw_star_sparkle(draw, int(w * 0.5), int(h * 0.08 + dy), size=16, color=(244, 114, 182, 240), angle=f*45)
            
        frames.append(frame)
        
    save_animation_pack(frames, "celebrate", "broomed-celebrate")


# ─── 4. CLEAN / POLISH & BUBBLES ANIMATION ──────────────────────
def generate_clean_animation():
    src_path = os.path.join(BASE_DIR, "broomed-clean.png")
    base_img = Image.open(src_path).convert("RGBA")
    w, h = base_img.size
    cx, cy = w / 2.0, h / 2.0
    pivot = (cx, h * 0.9)
    
    frames = []
    
    # Deterministic floating bubbles
    bubbles = [
        {"x": w * 0.22, "y0": h * 0.75, "r": 18, "speed": 65, "wobble": 12},
        {"x": w * 0.35, "y0": h * 0.65, "r": 12, "speed": 80, "wobble": -15},
        {"x": w * 0.78, "y0": h * 0.80, "r": 22, "speed": 55, "wobble": 18},
        {"x": w * 0.82, "y0": h * 0.55, "r": 15, "speed": 70, "wobble": -10},
        {"x": w * 0.15, "y0": h * 0.45, "r": 10, "speed": 90, "wobble": 8},
        {"x": w * 0.70, "y0": h * 0.35, "r": 14, "speed": 60, "wobble": -14},
    ]
    
    for f in range(8):
        t = f / 8.0 * 2.0 * math.pi
        
        # Rhythmic polishing sway
        angle = 2.5 * math.sin(t)
        dy = -4.0 * abs(math.sin(t))
        scale_y = 1.0 + 0.015 * math.sin(t * 2)
        scale_x = 1.0 / math.sqrt(scale_y)
        
        frame = transform_layer(base_img, dx=0, dy=dy, angle=angle, scale_x=scale_x, scale_y=scale_y, pivot=pivot)
        draw = ImageDraw.Draw(frame)
        
        # Render rising bubbles
        for b in bubbles:
            prog = (f / 8.0)
            by = b["y0"] - b["speed"] * prog
            bx = b["x"] + b["wobble"] * math.sin(prog * 2 * math.pi)
            draw_bubble(draw, bx, by, b["r"])
            
        # Squeaky clean star sparkle sweeping across handle & bristles
        sparkle_path = [
            (w * 0.48, h * 0.15, 6),
            (w * 0.52, h * 0.22, 14),
            (w * 0.55, h * 0.30, 22), # Apex flash
            (w * 0.50, h * 0.42, 16),
            (w * 0.45, h * 0.60, 10),
            (w * 0.38, h * 0.82, 18), # Bristle squeak
            (w * 0.65, h * 0.85, 12),
            (w * 0.50, h * 0.18, 8)
        ]
        sx, sy, ssize = sparkle_path[f]
        draw_star_sparkle(draw, int(sx), int(sy + dy), size=ssize, color=(255, 245, 160, 230), angle=f * 20)
        
        # Micro sparkle cluster on apex (frame 2)
        if f == 2:
            draw_star_sparkle(draw, int(sx - 25), int(sy - 15 + dy), size=8, color=(255, 255, 255, 220))
            draw_star_sparkle(draw, int(sx + 20), int(sy + 20 + dy), size=7, color=(186, 230, 253, 220))
            
        frames.append(frame)
        
    save_animation_pack(frames, "clean", "broomed-clean")


# ─── 5. FAST / SONIC DASH & SPEED TRAILS ANIMATION ──────────────
def generate_fast_animation():
    src_path = os.path.join(BASE_DIR, "broomed-fast.png")
    base_img = Image.open(src_path).convert("RGBA")
    w, h = base_img.size
    cx, cy = w / 2.0, h / 2.0
    pivot = (cx, h * 0.85)
    
    frames = []
    
    # Speed lines definition
    random.seed(99)
    speed_lines = []
    for _ in range(12):
        speed_lines.append({
            "y": random.uniform(h * 0.1, h * 0.9),
            "len": random.uniform(w * 0.25, w * 0.55),
            "thick": random.randint(2, 4),
            "speed": random.uniform(w * 0.6, w * 1.0)
        })
        
    for f in range(8):
        t = f / 8.0 * 2.0 * math.pi
        
        # High speed bounce and intense forward aerodynamic tilt
        dy = 5.0 * math.sin(t * 2)
        dx = -4.0 * math.cos(t)
        angle = -1.5 * math.sin(t)
        
        # 1. Background Motion Ghost Trail (Afterimages)
        canvas = Image.new("RGBA", (w, h), (0, 0, 0, 0))
        
        # Trailing ghost 2 (20% opacity, shifted back +45px)
        ghost2 = base_img.copy()
        alpha2 = ghost2.split()[-1]
        ghost2.putalpha(Image.eval(alpha2, lambda a: int(a * 0.20)))
        g2_trans = transform_layer(ghost2, dx=dx + 45, dy=dy + 3, angle=angle - 1.0, pivot=pivot)
        canvas.paste(g2_trans, (0, 0), g2_trans)
        
        # Trailing ghost 1 (38% opacity, shifted back +22px)
        ghost1 = base_img.copy()
        alpha1 = ghost1.split()[-1]
        ghost1.putalpha(Image.eval(alpha1, lambda a: int(a * 0.38)))
        g1_trans = transform_layer(ghost1, dx=dx + 22, dy=dy + 1.5, angle=angle - 0.5, pivot=pivot)
        canvas.paste(g1_trans, (0, 0), g1_trans)
        
        # 2. Main Character
        main_trans = transform_layer(base_img, dx=dx, dy=dy, angle=angle, pivot=pivot)
        canvas.paste(main_trans, (0, 0), main_trans)
        
        # 3. Dynamic Rushing Speed Lines
        draw = ImageDraw.Draw(canvas)
        for sl in speed_lines:
            prog = (f / 8.0)
            lx0 = (w * 1.1) - ((prog * sl["speed"] + sl["y"]) % (w * 1.3))
            lx1 = lx0 - sl["len"]
            ly = sl["y"]
            
            # Draw speed streak (gradient fading backwards)
            draw.line([(lx0, ly), (lx1, ly)], fill=(255, 255, 255, 180), width=sl["thick"])
            # Yellow kinetic glow streak behind
            draw.line([(lx0 - 10, ly + 1), (lx1 + 20, ly + 1)], fill=(251, 191, 36, 140), width=sl["thick"] + 1)
            
        # Trailing speed sparkles
        draw_star_sparkle(draw, int(w * 0.85 + dx), int(h * 0.55 + dy), size=8, color=(251, 191, 36, 220))
        draw_star_sparkle(draw, int(w * 0.90 + dx), int(h * 0.70 + dy), size=6, color=(56, 189, 248, 220))
        
        frames.append(canvas)
        
    save_animation_pack(frames, "fast", "broomed-fast")


# ─── 6. READING / STUDY & PAGE TURN ANIMATION ───────────────────
def generate_reading_animation():
    src_path = os.path.join(BASE_DIR, "broomed-reading.png")
    base_img = Image.open(src_path).convert("RGBA")
    w, h = base_img.size
    cx, cy = w / 2.0, h / 2.0
    pivot = (cx, h * 0.88)
    
    frames = []
    
    for f in range(8):
        t = f / 8.0 * 2.0 * math.pi
        
        # Gentle studious nod & posture
        dy = -3.0 * math.sin(t)
        angle = 1.0 * math.cos(t)
        
        frame = transform_layer(base_img, dx=0, dy=dy, angle=angle, pivot=pivot)
        draw = ImageDraw.Draw(frame)
        
        # Book coordinates: ~ x: 180..420, y: 620..780
        bx = int(w * 0.5)
        by = int(h * 0.68 + dy)
        book_w = int(w * 0.35)
        book_h = int(h * 0.12)
        
        # Dynamic 3D Page Turn Simulation across frames 3, 4, 5
        if f in (2, 3, 4, 5):
            turn_prog = (f - 2) / 3.0 # 0.0 to 1.0
            page_angle = math.pi * (1.0 - turn_prog) # from right to left
            
            # Page arc
            page_tip_x = bx + int(book_w * 0.45 * math.cos(page_angle))
            page_tip_y = by - int(book_h * 0.7 * math.sin(page_angle))
            
            # Draw curled page polygon
            page_poly = [
                (bx, by - book_h//2),
                (page_tip_x, page_tip_y - book_h//2),
                (page_tip_x, page_tip_y + book_h//2),
                (bx, by + book_h//2)
            ]
            draw.polygon(page_poly, fill=(250, 245, 235, 245), outline=(100, 70, 40, 220))
            # Page text lines hint
            draw.line([(bx + 5, by - 5), (page_tip_x - 5, page_tip_y - 5)], fill=(150, 130, 110, 160), width=2)
            draw.line([(bx + 5, by + 5), (page_tip_x - 5, page_tip_y + 5)], fill=(150, 130, 110, 160), width=2)
            
        # Comprehension Eureka Sparkle on frame 6 & 7
        if f in (6, 7):
            eye_spark_x = int(w * 0.56)
            eye_spark_y = int(h * 0.28 + dy)
            draw_star_sparkle(draw, eye_spark_x, eye_spark_y, size=12 if f == 6 else 7, color=(251, 191, 36, 240))
            
        frames.append(frame)
        
    save_animation_pack(frames, "reading", "broomed-reading")


# ─── 7. SEARCH / MAGNIFYING GLASS & SCAN ANIMATION ──────────────
def generate_search_animation():
    src_path = os.path.join(BASE_DIR, "broomed-search.png")
    base_img = Image.open(src_path).convert("RGBA")
    w, h = base_img.size
    cx, cy = w / 2.0, h / 2.0
    pivot = (cx, h * 0.88)
    
    frames = []
    
    for f in range(8):
        t = f / 8.0 * 2.0 * math.pi
        
        # Searching bob & sweep tilt
        dy = -4.0 * math.sin(t)
        angle = 2.0 * math.sin(t)
        
        frame = transform_layer(base_img, dx=0, dy=dy, angle=angle, pivot=pivot)
        draw = ImageDraw.Draw(frame)
        
        # Magnifying glass sweep path: sweeps from left to right
        sweep_x = int(w * 0.35 + (w * 0.30) * (0.5 + 0.5 * math.sin(t)))
        sweep_y = int(h * 0.62 + dy + 15 * math.cos(t))
        lens_r = int(w * 0.12)
        
        # Glowing radar / search scan ring around the lens
        draw.ellipse(
            (sweep_x - lens_r, sweep_y - lens_r, sweep_x + lens_r, sweep_y + lens_r),
            outline=(56, 189, 248, 190),
            width=3
        )
        
        # Lens glass reflection sheen
        draw.arc(
            (sweep_x - lens_r*0.8, sweep_y - lens_r*0.8, sweep_x + lens_r*0.8, sweep_y + lens_r*0.8),
            start=210, end=310,
            fill=(255, 255, 255, 220),
            width=4
        )
        
        # Hidden clue discovered on frame 3, 4, 5
        if f in (3, 4, 5):
            clue_size = 14 if f == 4 else 9
            draw_star_sparkle(draw, sweep_x, sweep_y, size=clue_size, color=(251, 191, 36, 255), angle=f*45)
            # Discovery ping ring
            draw.ellipse(
                (sweep_x - lens_r*1.3, sweep_y - lens_r*1.3, sweep_x + lens_r*1.3, sweep_y + lens_r*1.3),
                outline=(254, 240, 138, 140),
                width=2
            )
            
        frames.append(frame)
        
    save_animation_pack(frames, "search", "broomed-search")


# ─── 8. SHIELD / CYBER DEFENSE BARRIER ANIMATION ────────────────
def generate_shield_animation():
    src_path = os.path.join(BASE_DIR, "broomed-shield.png")
    base_img = Image.open(src_path).convert("RGBA")
    w, h = base_img.size
    cx, cy = w / 2.0, h / 2.0
    pivot = (cx, h * 0.9)
    
    frames = []
    
    for f in range(8):
        t = f / 8.0 * 2.0 * math.pi
        
        # Sturdy defensive brace pulse
        dy = -3.0 * math.sin(t)
        scale_x = 1.0 + 0.02 * math.sin(t)
        scale_y = 1.0 - 0.01 * math.sin(t)
        
        frame = transform_layer(base_img, dx=0, dy=dy, scale_x=scale_x, scale_y=scale_y, pivot=pivot)
        draw = ImageDraw.Draw(frame)
        
        # Shield location: ~ x: 420..820, y: 350..850
        sx = int(w * 0.62)
        sy = int(h * 0.58 + dy)
        sr = int(w * 0.28)
        
        # Expanding cyber defense forcefield wave (frames 2..6)
        wave_prog = (f / 8.0)
        wave_r = int(sr * (0.8 + 0.6 * wave_prog))
        wave_alpha = int(220 * (1.0 - wave_prog))
        
        # Outer expanding hexagonal forcefield barrier
        points = []
        for i in range(6):
            ang = i * (math.pi / 3.0) + math.pi / 6.0
            px = sx + wave_r * math.cos(ang)
            py = sy + wave_r * 0.9 * math.sin(ang)
            points.append((px, py))
            
        draw.polygon(points, outline=(16, 185, 129, wave_alpha), width=3)
        
        # Hexagonal grid nodes
        for px, py in points:
            draw.ellipse((px - 3, py - 3, px + 3, py + 3), fill=(110, 231, 183, wave_alpha))
            
        # Shield Rim Specular Glint (sliding shine)
        shine_angle = (f / 8.0) * math.pi * 2
        shine_x = sx + int(sr * 0.9 * math.cos(shine_angle))
        shine_y = sy + int(sr * 0.8 * math.sin(shine_angle))
        draw_star_sparkle(draw, shine_x, shine_y, size=10, color=(255, 255, 255, 230))
        
        # Deflection micro-sparks on apex (frame 3)
        if f == 3:
            draw_star_sparkle(draw, sx - 20, sy - 40, size=8, color=(52, 211, 153, 240))
            draw_star_sparkle(draw, sx + 30, sy + 50, size=7, color=(167, 243, 208, 240))
            
        frames.append(frame)
        
    save_animation_pack(frames, "shield", "broomed-shield")


# ─── 9. SLEEPING / SNORE & FLOATING ZZZ ANIMATION ───────────────
def generate_sleeping_animation():
    src_path = os.path.join(BASE_DIR, "broomed-sleeping.png")
    base_img = Image.open(src_path).convert("RGBA")
    w, h = base_img.size
    cx, cy = w / 2.0, h / 2.0
    pivot = (cx, h * 0.92)
    
    frames = []
    
    for f in range(8):
        t = f / 8.0 * 2.0 * math.pi
        
        # Slow deep breathing cycle (rise on inhale frames 1-4, sink on exhale frames 5-8)
        dy = -6.0 * math.sin(t)
        scale_y = 1.0 + 0.03 * math.sin(t)
        scale_x = 1.0 / math.sqrt(scale_y)
        angle = 1.5 * math.cos(t)
        
        frame = transform_layer(base_img, dx=0, dy=dy, angle=angle, scale_x=scale_x, scale_y=scale_y, pivot=pivot)
        draw = ImageDraw.Draw(frame)
        
        # Floating Zzz path: emerges from mouth/head (~ x: 0.55w, y: 0.35h) and rises upward
        z_origin_x = int(w * 0.58)
        z_origin_y = int(h * 0.32 + dy)
        
        # 3 sequential 'Z' particles along a rising S-curve
        for z_idx in range(3):
            # Staggered phase offset for each Z
            z_phase = (f / 8.0 + z_idx * 0.33) % 1.0
            
            # Rising S-curve trajectory
            zy = z_origin_y - int(z_phase * h * 0.35)
            zx = z_origin_x + int(35 * math.sin(z_phase * 2.5 * math.pi)) + z_idx * 15
            
            # Size grows as it rises, opacity fades as it nears the top
            z_size = int(14 + z_phase * 22)
            z_alpha = int(240 * (1.0 - (z_phase ** 1.5)))
            
            if z_alpha > 20:
                draw_letter_z(draw, zx, zy, z_size, (147, 197, 253, z_alpha), width=max(2, int(z_size * 0.18)))
                
        # Snooze breath bubble at mouth on lowest point (frames 6, 7)
        if f in (6, 7):
            bubble_r = 10 if f == 6 else 6
            draw_bubble(draw, z_origin_x - 15, z_origin_y + 10, bubble_r, alpha=160)
            
        frames.append(frame)
        
    save_animation_pack(frames, "sleeping", "broomed-sleeping")


# ─── Main Pipeline ──────────────────────────────────────────────
def main():
    print("=" * 60)
    print("Generating 8-Frame Animations for all 9 Mascot Assets...")
    print("=" * 60)
    
    generators = [
        ("1. Reference / Idle", generate_reference_animation),
        ("2. AI / Hologram", generate_ai_animation),
        ("3. Celebrate / Party", generate_celebrate_animation),
        ("4. Clean / Polish", generate_clean_animation),
        ("5. Fast / Sonic Dash", generate_fast_animation),
        ("6. Reading / Book Study", generate_reading_animation),
        ("7. Search / Detective", generate_search_animation),
        ("8. Shield / Defense Barrier", generate_shield_animation),
        ("9. Sleeping / Snore Breath", generate_sleeping_animation)
    ]
    
    for name, gen_fn in generators:
        print(f"\n--- Running Generator: {name} ---")
        gen_fn()
        
    print("\n" + "=" * 60)
    print("[SUCCESS] All 9 8-Frame Animation Suites Generated Successfully!")
    print("=" * 60)

if __name__ == "__main__":
    main()
