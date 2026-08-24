/**
 * Authentic 8-bit / 16-bit Retro Pixel Art Mascot Generator (v2 Refined)
 * Operates on a 96x96 pixel grid (upscaled 4x to 384x384 SVG with crispEdges).
 */

const fs = require('fs');
const path = require('path');

const IDLE_GRIDS = JSON.parse(fs.readFileSync(path.join(__dirname, 'idle_grids.json'), 'utf8'));

const PALETTE = {
  black: '#010104',
  blackSoft: '#150D08',
  darkBrown: '#533926',
  woodShadow: '#63340F',
  woodMid: '#743F16',
  woodLight: '#946D42',
  woodHighlight: '#AF763E',
  strawDark: '#63340F',
  strawMid: '#743F16',
  strawLight: '#B0773E',
  strawHighlight: '#C68B4E',
  metalDark: '#526068',
  metalMid: '#787C7D',
  metalLight: '#95A8B3',
  metalHigh: '#CCD7DA',
  white: '#FEFEFE',
  whiteShade: '#ADBBC1',
  gloveShade: '#828D93',
  cheekBlush: '#FCA5A5',
  
  // Accents
  coffeeCup: '#FAF5EE',
  coffeeCupShade: '#DCD4C8',
  coffeeLiquid: '#3E2312',
  coffeeGold: '#D4A23A',
  coffeeSteam: '#FAF5EE',
  coffeeSteamFade: '#CAD4DB',

  bookRed: '#D64545',
  bookRedDark: '#9E2828',
  bookGreen: '#10B981',
  bookGreenDark: '#047857',
  bookBlue: '#3B82F6',
  bookBlueDark: '#1D4ED8',
  bookAmber: '#F59E0B',
  bookAmberDark: '#B45309',
  bookPurple: '#8B5CF6',
  bookPurpleDark: '#6D28D9',
  shelfWood: '#5C381E',
  shelfWoodDark: '#3A200E',
  shelfWoodLight: '#8A5832',

  sparkleGold: '#FBBF24',
  sparkleBright: '#FEF08A',
  sparkleCyan: '#38BDF8',
  sparkleCyanLight: '#7DD3FC',

  bucketMetal: '#64748B',
  bucketMetalLight: '#94A3B8',
  bucketMetalDark: '#334155',
  bucketRim: '#CBD5E1',
  foamWhite: '#F8FAFC',
  foamBubble: '#BAE6FD',
  foamShadow: '#93C5FD'
};

function cloneGrid(grid) {
  return grid.map(row => [...row]);
}

function createEmptyGrid() {
  return Array.from({length: 96}, () => Array(96).fill(null));
}

function setPixel(grid, x, y, color) {
  if (x >= 0 && x < 96 && y >= 0 && y < 96) {
    grid[y][x] = color;
  }
}

function fillRect(grid, x, y, w, h, color) {
  for (let dy = 0; dy < h; dy++) {
    for (let dx = 0; dx < w; dx++) {
      setPixel(grid, x + dx, y + dy, color);
    }
  }
}

function clearRect(grid, x, y, w, h) {
  for (let dy = 0; dy < h; dy++) {
    for (let dx = 0; dx < w; dx++) {
      setPixel(grid, x + dx, y + dy, null);
    }
  }
}

function drawLine(grid, x0, y0, x1, y1, color) {
  let dx = Math.abs(x1 - x0), sx = x0 < x1 ? 1 : -1;
  let dy = -Math.abs(y1 - y0), sy = y0 < y1 ? 1 : -1;
  let err = dx + dy, e2;
  while (true) {
    setPixel(grid, x0, y0, color);
    if (x0 === x1 && y0 === y1) break;
    e2 = 2 * err;
    if (e2 >= dy) { err += dy; x0 += sx; }
    if (e2 <= dx) { err += dx; y0 += sy; }
  }
}

function shiftGrid(grid, shiftX, shiftY) {
  const newGrid = createEmptyGrid();
  for (let y = 0; y < 96; y++) {
    for (let x = 0; x < 96; x++) {
      if (grid[y][x]) {
        const nx = x + shiftX;
        const ny = y + shiftY;
        if (nx >= 0 && nx < 96 && ny >= 0 && ny < 96) {
          newGrid[ny][nx] = grid[y][x];
        }
      }
    }
  }
  return newGrid;
}

// Clear original hanging arms completely
function removeOriginalArms(grid) {
  // Left arm: x: 14..37, y: 50..74
  clearRect(grid, 14, 50, 24, 25);
  // Right arm: x: 59..84, y: 50..74
  clearRect(grid, 59, 50, 26, 25);
}

function gridToPixelSvg(grid) {
  let rects = [];
  for (let y = 0; y < 96; y++) {
    let startX = null;
    let currentColor = null;

    for (let x = 0; x < 96; x++) {
      const col = grid[y][x];
      if (col === currentColor) {
        // continue
      } else {
        if (currentColor && startX !== null) {
          const w = x - startX;
          rects.push(`<rect x="${startX * 4}" y="${y * 4}" width="${w * 4}" height="4" fill="${currentColor}"/>`);
        }
        startX = col ? x : null;
        currentColor = col;
      }
    }
    if (currentColor && startX !== null) {
      const w = 96 - startX;
      rects.push(`<rect x="${startX * 4}" y="${y * 4}" width="${w * 4}" height="4" fill="${currentColor}"/>`);
    }
  }

  return `<?xml version="1.0" encoding="UTF-8"?>
<svg version="1.1" xmlns="http://www.w3.org/2000/svg" width="384" height="384" viewBox="0 0 384 384" shape-rendering="crispEdges">
${rects.join('\n')}
</svg>`;
}

// Replace Eye in grid (Eye is located at x: 44..52, y: 23..31)
function setEyeState(grid, state = 'open', lookX = 0, lookY = 0) {
  clearRect(grid, 44, 23, 10, 9);

  if (state === 'sleeping' || state === 'blink') {
    // Brow
    fillRect(grid, 44, 23, 9, 2, PALETTE.black);
    // Closed curved line (⌒)
    setPixel(grid, 44, 27, PALETTE.black);
    setPixel(grid, 45, 28, PALETTE.black);
    fillRect(grid, 46, 29, 5, 1, PALETTE.black);
    setPixel(grid, 51, 28, PALETTE.black);
    setPixel(grid, 52, 27, PALETTE.black);
    // Cheeks
    fillRect(grid, 43, 30, 2, 1, PALETTE.cheekBlush);
    fillRect(grid, 52, 30, 2, 1, PALETTE.cheekBlush);
    return;
  }

  if (state === 'happy') {
    // Brow
    fillRect(grid, 44, 22, 9, 2, PALETTE.black);
    // Happy arch (^^)
    fillRect(grid, 46, 26, 5, 1, PALETTE.black);
    setPixel(grid, 45, 27, PALETTE.black);
    setPixel(grid, 51, 27, PALETTE.black);
    setPixel(grid, 44, 28, PALETTE.black);
    setPixel(grid, 52, 28, PALETTE.black);
    // Cheeks
    fillRect(grid, 43, 30, 2, 1, PALETTE.cheekBlush);
    fillRect(grid, 52, 30, 2, 1, PALETTE.cheekBlush);
    return;
  }

  // Open Eye
  fillRect(grid, 44, 23, 9, 2, PALETTE.black);
  fillRect(grid, 44, 25, 9, 7, PALETTE.black);
  fillRect(grid, 45, 26, 7, 5, PALETTE.white);
  fillRect(grid, 45, 26, 2, 5, PALETTE.whiteShade);
  
  // Pupil
  const px = Math.min(49, Math.max(46, 48 + lookX));
  const py = Math.min(29, Math.max(26, 28 + lookY));
  fillRect(grid, px - 1, py - 1, 3, 3, PALETTE.black);
  setPixel(grid, px, py - 1, PALETTE.white); // glint
}

// ─── 1. COFFEE STATE (7 FRAMES) ──────────────────────────────────
function generateCoffeeState() {
  const frames = [];
  const baseFrames = [2, 3, 4, 5, 6, 7, 8].map(i => IDLE_GRIDS[`idle-${i}`]);

  for (let f = 1; f <= 7; f++) {
    const grid = cloneGrid(baseFrames[f - 1]);
    removeOriginalArms(grid);

    let mugY = 56;
    let eyeState = 'open';
    let lookX = 0, lookY = 0;

    if (f === 1) { mugY = 56; eyeState = 'open'; lookX = 0; lookY = 1; }
    if (f === 2) { mugY = 55; eyeState = 'open'; lookX = 0; lookY = 2; }
    if (f === 3) { mugY = 51; eyeState = 'happy'; lookX = 0; lookY = 1; }
    if (f === 4) { mugY = 47; eyeState = 'sleeping'; lookX = 0; lookY = 0; } // sipping
    if (f === 5) { mugY = 51; eyeState = 'happy'; lookX = 1; lookY = 0; }
    if (f === 6) { mugY = 55; eyeState = 'open'; lookX = 1; lookY = -1; }
    if (f === 7) { mugY = 56; eyeState = 'open'; lookX = 0; lookY = 0; }

    setEyeState(grid, eyeState, lookX, lookY);

    // Draw Hands & Ceramic Coffee Mug
    const mx = 43;
    const my = mugY;

    // Ceramic Mug
    fillRect(grid, mx, my, 10, 7, PALETTE.black);
    fillRect(grid, mx + 1, my + 1, 8, 5, PALETTE.coffeeCup);
    fillRect(grid, mx + 1, my + 1, 2, 5, PALETTE.coffeeCupShade);
    // Gold emblem
    fillRect(grid, mx + 4, my + 3, 2, 2, PALETTE.coffeeGold);
    // Mug Handle
    fillRect(grid, mx + 10, my + 2, 3, 4, PALETTE.black);
    fillRect(grid, mx + 10, my + 3, 2, 2, PALETTE.coffeeCup);
    setPixel(grid, mx + 10, my + 3, null); // handle hole
    // Coffee Surface
    fillRect(grid, mx, my, 10, 2, PALETTE.black);
    fillRect(grid, mx + 1, my, 8, 1, PALETTE.coffeeLiquid);
    setPixel(grid, mx + 2, my, PALETTE.white); // shine

    // White Gloves Holding Mug
    // Left Glove
    fillRect(grid, mx - 5, my + 1, 6, 6, PALETTE.black);
    fillRect(grid, mx - 4, my + 2, 4, 4, PALETTE.white);
    fillRect(grid, mx - 5, my + 3, 2, 3, PALETTE.whiteShade);
    setPixel(grid, mx, my + 2, PALETTE.white);
    setPixel(grid, mx + 1, my + 4, PALETTE.white);

    // Right Glove
    fillRect(grid, mx + 8, my + 1, 6, 6, PALETTE.black);
    fillRect(grid, mx + 8, my + 2, 4, 4, PALETTE.white);
    fillRect(grid, mx + 10, my + 3, 2, 3, PALETTE.whiteShade);
    setPixel(grid, mx + 8, my + 2, PALETTE.white);
    setPixel(grid, mx + 7, my + 4, PALETTE.white);

    // Arms
    drawLine(grid, 37, 52, mx - 4, my + 3, PALETTE.black);
    drawLine(grid, 58, 52, mx + 10, my + 3, PALETTE.black);

    // Steam Wisps
    const steamPatterns = [
      [[46, my-2], [47, my-3], [48, my-4], [47, my-5], [46, my-6], [49, my-2], [50, my-3], [49, my-4]],
      [[46, my-3], [47, my-4], [48, my-5], [47, my-6], [48, my-7], [49, my-3], [50, my-4], [51, my-5]],
      [[47, my-3], [48, my-4], [49, my-5], [48, my-6], [47, my-7], [50, my-4], [51, my-5]],
      [[47, my-2], [48, my-3], [49, my-3], [50, my-2]], // low steam on sip
      [[46, my-2], [47, my-3], [48, my-4], [49, my-5], [48, my-6], [47, my-7], [50, my-3], [51, my-4]],
      [[45, my-3], [46, my-4], [47, my-5], [46, my-6], [47, my-7], [48, my-8], [50, my-3], [51, my-4]],
      [[46, my-2], [47, my-3], [48, my-4], [47, my-5], [46, my-6], [49, my-2], [50, my-3], [50, my-4]]
    ];

    const steam = steamPatterns[(f - 1) % 7];
    steam.forEach(([sx, sy], idx) => {
      const col = idx > 4 ? PALETTE.coffeeSteamFade : PALETTE.coffeeSteam;
      setPixel(grid, sx, sy, col);
    });

    frames.push(gridToPixelSvg(grid));
  }

  return frames;
}

// ─── 2. BROOMING STATE (7 FRAMES) ────────────────────────────────
function generateBroomingState() {
  const frames = [];
  const baseFrames = [2, 3, 4, 5, 6, 7, 8].map(i => IDLE_GRIDS[`idle-${i}`]);

  for (let f = 1; f <= 7; f++) {
    const shiftSettings = [
      { sx: -4, sy: 0, eye: 'open', lx: 1, ly: 1, swoosh: 'right_start', sparkles: [] },
      { sx: 2,  sy: 1, eye: 'open', lx: 2, ly: 2, swoosh: 'right_sweep', sparkles: [[74, 82, 3]] },
      { sx: 7,  sy: 2, eye: 'happy',lx: 2, ly: 1, swoosh: 'right_burst', sparkles: [[78, 76, 4], [84, 83, 3], [72, 85, 2]] },
      { sx: 2,  sy: 0, eye: 'open', lx: 1, ly: 1, swoosh: 'none',        sparkles: [[80, 75, 2]] },
      { sx: -3, sy: 1, eye: 'open', lx: -2, ly: 2, swoosh: 'left_sweep',  sparkles: [[22, 82, 3]] },
      { sx: -7, sy: 2, eye: 'happy',lx: -2, ly: 1, swoosh: 'left_burst',  sparkles: [[18, 76, 4], [12, 83, 3], [24, 85, 2]] },
      { sx: -1, sy: 0, eye: 'open', lx: 0, ly: 1, swoosh: 'none',        sparkles: [[16, 75, 2]] }
    ][f - 1];

    let grid = cloneGrid(baseFrames[f - 1]);
    removeOriginalArms(grid);

    // Draw Sweeping Stance Arms gripping handle
    fillRect(grid, 42, 48, 6, 6, PALETTE.black);
    fillRect(grid, 43, 49, 4, 4, PALETTE.white);
    fillRect(grid, 50, 54, 6, 6, PALETTE.black);
    fillRect(grid, 51, 55, 4, 4, PALETTE.white);
    drawLine(grid, 37, 52, 42, 50, PALETTE.black);
    drawLine(grid, 58, 56, 54, 56, PALETTE.black);

    // Shift broom character
    grid = shiftGrid(grid, shiftSettings.sx, shiftSettings.sy);

    // Floor Line
    drawLine(grid, 8, 88, 88, 88, PALETTE.black);
    drawLine(grid, 10, 89, 86, 89, PALETTE.metalDark);

    // Sweeping Action Swoosh Trails
    if (shiftSettings.swoosh === 'right_sweep' || shiftSettings.swoosh === 'right_burst') {
      drawLine(grid, 38, 86, 76, 85, PALETTE.metalLight);
      drawLine(grid, 44, 87, 72, 87, PALETTE.white);
      drawLine(grid, 50, 84, 78, 83, PALETTE.sparkleBright);
    } else if (shiftSettings.swoosh === 'left_sweep' || shiftSettings.swoosh === 'left_burst') {
      drawLine(grid, 58, 86, 20, 85, PALETTE.metalLight);
      drawLine(grid, 52, 87, 24, 87, PALETTE.white);
      drawLine(grid, 46, 84, 18, 83, PALETTE.sparkleBright);
    }

    // Sparkles
    shiftSettings.sparkles.forEach(([spX, spY, size]) => {
      setPixel(grid, spX, spY, PALETTE.white);
      setPixel(grid, spX - 1, spY, PALETTE.sparkleGold);
      setPixel(grid, spX + 1, spY, PALETTE.sparkleGold);
      setPixel(grid, spX, spY - 1, PALETTE.sparkleGold);
      setPixel(grid, spX, spY + 1, PALETTE.sparkleGold);
      if (size > 2) {
        setPixel(grid, spX - 2, spY, PALETTE.sparkleBright);
        setPixel(grid, spX + 2, spY, PALETTE.sparkleBright);
        setPixel(grid, spX, spY - 2, PALETTE.sparkleBright);
        setPixel(grid, spX, spY + 2, PALETTE.sparkleBright);
      }
    });

    setEyeState(grid, shiftSettings.eye, shiftSettings.lx, shiftSettings.ly);
    frames.push(gridToPixelSvg(grid));
  }

  return frames;
}

// ─── 3. BOOKSHELF STATE (7 FRAMES) ───────────────────────────────
function generateBookshelfState() {
  const frames = [];
  const baseFrames = [2, 3, 4, 5, 6, 7, 8].map(i => IDLE_GRIDS[`idle-${i}`]);

  for (let f = 1; f <= 7; f++) {
    let grid = cloneGrid(baseFrames[f - 1]);
    
    // Clear right arm so it can interact with the shelf
    clearRect(grid, 59, 50, 26, 25);

    // Shift mascot left
    grid = shiftGrid(grid, -10, 0);

    const shelfX = 60;
    const shelfY = 70;

    // Bookshelf Plank
    fillRect(grid, shelfX - 2, shelfY, 34, 4, PALETTE.black);
    fillRect(grid, shelfX - 1, shelfY + 1, 32, 2, PALETTE.shelfWood);
    fillRect(grid, shelfX - 1, shelfY + 1, 32, 1, PALETTE.shelfWoodLight);
    fillRect(grid, shelfX + 4, shelfY + 4, 3, 4, PALETTE.shelfWoodDark);
    fillRect(grid, shelfX + 24, shelfY + 4, 3, 4, PALETTE.shelfWoodDark);

    // Book 1: Red (h: 15)
    fillRect(grid, shelfX + 1, shelfY - 15, 4, 15, PALETTE.black);
    fillRect(grid, shelfX + 2, shelfY - 14, 2, 14, PALETTE.bookRed);
    setPixel(grid, shelfX + 2, shelfY - 12, PALETTE.sparkleGold);
    setPixel(grid, shelfX + 2, shelfY - 4, PALETTE.sparkleGold);

    // Book 2: Green (h: 13)
    fillRect(grid, shelfX + 6, shelfY - 13, 4, 13, PALETTE.black);
    fillRect(grid, shelfX + 7, shelfY - 12, 2, 12, PALETTE.bookGreen);
    fillRect(grid, shelfX + 7, shelfY - 10, 2, 3, PALETTE.white);

    // Book 3: Blue (tilting or straight)
    if (f === 1) {
      drawLine(grid, shelfX + 11, shelfY - 1, shelfX + 15, shelfY - 14, PALETTE.black);
      drawLine(grid, shelfX + 12, shelfY - 1, shelfX + 16, shelfY - 14, PALETTE.bookBlue);
      drawLine(grid, shelfX + 13, shelfY - 1, shelfX + 17, shelfY - 14, PALETTE.bookBlue);
      drawLine(grid, shelfX + 14, shelfY - 1, shelfX + 18, shelfY - 14, PALETTE.black);
    } else {
      fillRect(grid, shelfX + 11, shelfY - 14, 4, 14, PALETTE.black);
      fillRect(grid, shelfX + 12, shelfY - 13, 2, 13, PALETTE.bookBlue);
      setPixel(grid, shelfX + 12, shelfY - 11, PALETTE.sparkleGold);
      setPixel(grid, shelfX + 12, shelfY - 9, PALETTE.sparkleGold);
    }

    // Book 4: Amber (Inserted in f >= 5, or hovering in f === 4)
    if (f >= 5) {
      fillRect(grid, shelfX + 16, shelfY - 12, 4, 12, PALETTE.black);
      fillRect(grid, shelfX + 17, shelfY - 11, 2, 11, PALETTE.bookAmber);
      setPixel(grid, shelfX + 17, shelfY - 9, PALETTE.white);
    } else if (f === 4) {
      fillRect(grid, shelfX + 15, shelfY - 20, 4, 12, PALETTE.black);
      fillRect(grid, shelfX + 16, shelfY - 19, 2, 11, PALETTE.bookAmber);
    }

    // Book 5: Purple (h: 15)
    fillRect(grid, shelfX + 21, shelfY - 15, 4, 15, PALETTE.black);
    fillRect(grid, shelfX + 22, shelfY - 14, 2, 14, PALETTE.bookPurple);
    setPixel(grid, shelfX + 22, shelfY - 12, PALETTE.sparkleGold);
    setPixel(grid, shelfX + 22, shelfY - 4, PALETTE.sparkleGold);

    // Bookend
    fillRect(grid, shelfX + 28, shelfY - 9, 2, 9, PALETTE.black);
    fillRect(grid, shelfX + 28, shelfY - 1, 4, 2, PALETTE.black);
    setPixel(grid, shelfX + 28, shelfY - 8, PALETTE.metalLight);

    // Mascot Glove Reaching
    let hx = shelfX + 12;
    let hy = shelfY - 8;
    if (f === 1) { hx = shelfX + 8; hy = shelfY - 8; }
    if (f === 2) { hx = shelfX + 12; hy = shelfY - 8; }
    if (f === 3) { hx = shelfX + 14; hy = shelfY - 8; }
    if (f === 4) { hx = shelfX + 14; hy = shelfY - 16; }
    if (f === 5) { hx = shelfX + 16; hy = shelfY - 10; }
    if (f === 6) { hx = shelfX + 18; hy = shelfY - 6; }
    if (f === 7) { hx = shelfX + 6; hy = shelfY - 2; }

    drawLine(grid, 48, 54, hx - 2, hy + 2, PALETTE.black);
    fillRect(grid, hx - 2, hy, 6, 6, PALETTE.black);
    fillRect(grid, hx - 1, hy + 1, 4, 4, PALETTE.white);
    setPixel(grid, hx + 3, hy + 2, PALETTE.white);

    // Sparkles
    if (f === 3) {
      setPixel(grid, shelfX + 13, shelfY - 18, PALETTE.white);
      setPixel(grid, shelfX + 12, shelfY - 18, PALETTE.sparkleGold);
      setPixel(grid, shelfX + 14, shelfY - 18, PALETTE.sparkleGold);
      setPixel(grid, shelfX + 13, shelfY - 19, PALETTE.sparkleGold);
      setPixel(grid, shelfX + 13, shelfY - 17, PALETTE.sparkleGold);
    } else if (f === 6) {
      setPixel(grid, shelfX + 18, shelfY - 16, PALETTE.white);
      setPixel(grid, shelfX + 17, shelfY - 16, PALETTE.sparkleGold);
      setPixel(grid, shelfX + 19, shelfY - 16, PALETTE.sparkleGold);
    } else if (f === 7) {
      setPixel(grid, shelfX + 25, shelfY - 18, PALETTE.white);
      setPixel(grid, shelfX + 24, shelfY - 18, PALETTE.sparkleGold);
      setPixel(grid, shelfX + 26, shelfY - 18, PALETTE.sparkleGold);
    }

    setEyeState(grid, (f === 3 || f === 6 || f === 7) ? 'happy' : 'open', 2, 1);
    frames.push(gridToPixelSvg(grid));
  }

  return frames;
}

// ─── 4. CHIN RUB / THINKING STATE (7 FRAMES) ─────────────────────
function generateThinkingState() {
  const frames = [];
  const baseFrames = [2, 3, 4, 5, 6, 7, 8].map(i => IDLE_GRIDS[`idle-${i}`]);

  for (let f = 1; f <= 7; f++) {
    const grid = cloneGrid(baseFrames[f - 1]);
    removeOriginalArms(grid);

    // Left Arm on Hip
    drawLine(grid, 38, 54, 30, 60, PALETTE.black);
    fillRect(grid, 27, 58, 6, 6, PALETTE.black);
    fillRect(grid, 28, 59, 4, 4, PALETTE.white);
    setPixel(grid, 26, 60, PALETTE.white);

    // Right Arm coming up to Chin (Chin at x: 48..52, y: 38..42)
    let chinHandY = 40;
    if (f === 2) chinHandY = 38;
    if (f === 3) chinHandY = 37;
    if (f === 4) chinHandY = 36;
    if (f === 5) chinHandY = 38;
    if (f === 6) chinHandY = 39;

    // Arm line
    drawLine(grid, 58, 54, 53, chinHandY + 4, PALETTE.black);
    // White Cartoon Glove on chin
    fillRect(grid, 48, chinHandY, 7, 7, PALETTE.black);
    fillRect(grid, 49, chinHandY + 1, 5, 5, PALETTE.white);
    fillRect(grid, 52, chinHandY + 2, 2, 4, PALETTE.whiteShade);
    // Extended index finger tapping chin
    drawLine(grid, 48, chinHandY + 2, 47, chinHandY - 2, PALETTE.black);
    fillRect(grid, 47, chinHandY - 1, 2, 3, PALETTE.white);

    // Thought Sparks & Eyes
    if (f === 1) {
      setEyeState(grid, 'open', -1, -2);
      fillRect(grid, 38, 16, 2, 2, PALETTE.sparkleCyan);
    } else if (f === 2) {
      setEyeState(grid, 'open', -2, -2);
      fillRect(grid, 37, 14, 2, 2, PALETTE.sparkleCyan);
      fillRect(grid, 32, 10, 3, 3, PALETTE.sparkleCyan);
    } else if (f === 3) {
      setEyeState(grid, 'open', -2, -2);
      fillRect(grid, 36, 13, 2, 2, PALETTE.sparkleCyan);
      fillRect(grid, 30, 8, 3, 3, PALETTE.sparkleCyan);
      fillRect(grid, 24, 2, 6, 6, PALETTE.black);
      fillRect(grid, 25, 3, 4, 4, PALETTE.sparkleCyanLight);
      setPixel(grid, 26, 4, PALETTE.sparkleGold);
    } else if (f === 4) {
      setEyeState(grid, 'sleeping', 0, 0); // thinking blink
      fillRect(grid, 23, 2, 8, 8, PALETTE.black);
      fillRect(grid, 24, 3, 6, 6, PALETTE.sparkleGold);
      fillRect(grid, 25, 4, 4, 4, PALETTE.sparkleBright);
    } else if (f === 5) {
      setEyeState(grid, 'open', 2, -2);
      setPixel(grid, 62, 10, PALETTE.white);
      setPixel(grid, 61, 10, PALETTE.sparkleGold);
      setPixel(grid, 63, 10, PALETTE.sparkleGold);
      setPixel(grid, 62, 9, PALETTE.sparkleGold);
      setPixel(grid, 62, 11, PALETTE.sparkleGold);
    } else if (f === 6) {
      setEyeState(grid, 'happy', 1, -1);
      // Eureka Star Spark! (✦)
      setPixel(grid, 64, 8, PALETTE.white);
      fillRect(grid, 61, 8, 7, 1, PALETTE.sparkleGold);
      fillRect(grid, 64, 5, 1, 7, PALETTE.sparkleGold);
      setPixel(grid, 63, 7, PALETTE.sparkleBright);
      setPixel(grid, 65, 7, PALETTE.sparkleBright);
      setPixel(grid, 63, 9, PALETTE.sparkleBright);
      setPixel(grid, 65, 9, PALETTE.sparkleBright);
    } else if (f === 7) {
      setEyeState(grid, 'open', 0, -1);
      setPixel(grid, 62, 14, PALETTE.sparkleGold);
    }

    frames.push(gridToPixelSvg(grid));
  }

  return frames;
}

// ─── 5. BROOM IN BUCKET (STATIC SCENE) ───────────────────────────
function generateBucketState() {
  const grid = createEmptyGrid();

  // Floor Shadow
  fillRect(grid, 26, 86, 44, 4, PALETTE.blackSoft);
  fillRect(grid, 30, 87, 36, 2, PALETTE.black);

  // Bucket Back Rim
  fillRect(grid, 28, 56, 40, 6, PALETTE.bucketMetalDark);
  fillRect(grid, 30, 57, 36, 4, '#1E293B');

  // Mascot tilted in bucket (tilted 15° back)
  for (let y = 18; y < 58; y++) {
    const hx = 44 + Math.round((58 - y) * 0.18);
    fillRect(grid, hx, y, 7, 1, PALETTE.black);
    fillRect(grid, hx + 1, y, 5, 1, PALETTE.woodMid);
    fillRect(grid, hx + 2, y, 2, 1, PALETTE.woodLight);
  }
  // Handle Top Knob
  fillRect(grid, 49, 14, 8, 6, PALETTE.black);
  fillRect(grid, 50, 15, 6, 4, PALETTE.woodLight);
  fillRect(grid, 52, 15, 2, 2, PALETTE.woodHighlight);

  // Sleeping Eye (y: 26)
  setPixel(grid, 49, 26, PALETTE.black);
  setPixel(grid, 50, 27, PALETTE.black);
  fillRect(grid, 51, 28, 4, 1, PALETTE.black);
  setPixel(grid, 55, 27, PALETTE.black);
  setPixel(grid, 56, 26, PALETTE.black);
  fillRect(grid, 48, 29, 2, 1, PALETTE.cheekBlush);
  fillRect(grid, 55, 29, 2, 1, PALETTE.cheekBlush);

  // Mascot Arm resting on bucket rim
  drawLine(grid, 46, 54, 34, 54, PALETTE.black);
  fillRect(grid, 31, 52, 6, 6, PALETTE.black);
  fillRect(grid, 32, 53, 4, 4, PALETTE.white);
  fillRect(grid, 31, 54, 2, 3, PALETTE.whiteShade);

  // Bucket Body (tapering from x: 26..70 to x: 32..64)
  for (let y = 60; y <= 86; y++) {
    const prog = (y - 60) / 26;
    const x0 = Math.round(26 + prog * 6);
    const x1 = Math.round(70 - prog * 6);
    const w = x1 - x0;
    
    fillRect(grid, x0, y, w, 1, PALETTE.black);
    fillRect(grid, x0 + 1, y, w - 2, 1, PALETTE.bucketMetal);
    fillRect(grid, x0 + 1, y, Math.round(w * 0.35), 1, PALETTE.bucketMetalDark);
    fillRect(grid, x0 + Math.round(w * 0.7), y, 2, 1, PALETTE.bucketMetalLight);
    fillRect(grid, x0 + Math.round(w * 0.85), y, 2, 1, PALETTE.bucketRim);
  }

  // Metal Hoops / Ribs on Bucket
  fillRect(grid, 28, 68, 40, 2, PALETTE.black);
  fillRect(grid, 29, 68, 38, 1, PALETTE.bucketRim);
  fillRect(grid, 31, 78, 34, 2, PALETTE.black);
  fillRect(grid, 32, 78, 32, 1, PALETTE.bucketRim);

  // Metal Handle Arc
  for (let a = 0; a <= 180; a += 10) {
    const rad = a * Math.PI / 180;
    const hx = Math.round(48 - Math.cos(rad) * 22);
    const hy = Math.round(58 - Math.sin(rad) * 18);
    fillRect(grid, hx, hy, 2, 2, PALETTE.black);
    setPixel(grid, hx, hy, PALETTE.metalLight);
  }
  // Wooden Handle Grip
  fillRect(grid, 44, 38, 8, 4, PALETTE.black);
  fillRect(grid, 45, 39, 6, 2, PALETTE.woodMid);

  // Bucket Front Lip
  fillRect(grid, 25, 58, 46, 3, PALETTE.black);
  fillRect(grid, 26, 59, 44, 2, PALETTE.bucketRim);

  // Suds Blobs
  fillRect(grid, 28, 56, 7, 5, PALETTE.black);
  fillRect(grid, 29, 57, 5, 3, PALETTE.foamWhite);
  fillRect(grid, 34, 54, 8, 6, PALETTE.black);
  fillRect(grid, 35, 55, 6, 4, PALETTE.foamBubble);
  fillRect(grid, 42, 55, 9, 6, PALETTE.black);
  fillRect(grid, 43, 56, 7, 4, PALETTE.foamWhite);
  fillRect(grid, 50, 54, 8, 6, PALETTE.black);
  fillRect(grid, 51, 55, 6, 4, PALETTE.foamBubble);
  fillRect(grid, 58, 56, 9, 5, PALETTE.black);
  fillRect(grid, 59, 57, 7, 3, PALETTE.foamWhite);

  // Soap Drips Down Bucket
  fillRect(grid, 35, 61, 3, 7, PALETTE.black);
  fillRect(grid, 36, 61, 1, 6, PALETTE.foamWhite);
  fillRect(grid, 55, 61, 3, 8, PALETTE.black);
  fillRect(grid, 56, 61, 1, 7, PALETTE.foamBubble);

  // Floating Soap Bubbles
  fillRect(grid, 20, 44, 5, 5, PALETTE.black);
  fillRect(grid, 21, 45, 3, 3, PALETTE.foamBubble);
  setPixel(grid, 21, 45, PALETTE.white);

  fillRect(grid, 24, 30, 6, 6, PALETTE.black);
  fillRect(grid, 25, 31, 4, 4, PALETTE.foamBubble);
  setPixel(grid, 25, 31, PALETTE.white);

  fillRect(grid, 70, 48, 5, 5, PALETTE.black);
  fillRect(grid, 71, 49, 3, 3, PALETTE.foamBubble);
  setPixel(grid, 71, 49, PALETTE.white);

  // Floating Pixel Z's
  // Big Z
  fillRect(grid, 62, 16, 6, 2, PALETTE.black);
  setPixel(grid, 66, 18, PALETTE.black);
  setPixel(grid, 65, 19, PALETTE.black);
  setPixel(grid, 64, 20, PALETTE.black);
  setPixel(grid, 63, 21, PALETTE.black);
  fillRect(grid, 62, 22, 6, 2, PALETTE.black);
  fillRect(grid, 63, 17, 4, 1, PALETTE.sparkleCyan);
  setPixel(grid, 65, 19, PALETTE.sparkleCyan);
  fillRect(grid, 63, 22, 4, 1, PALETTE.sparkleCyan);

  // Medium Z
  fillRect(grid, 70, 10, 5, 2, PALETTE.black);
  setPixel(grid, 73, 12, PALETTE.black);
  setPixel(grid, 72, 13, PALETTE.black);
  setPixel(grid, 71, 14, PALETTE.black);
  fillRect(grid, 70, 15, 5, 2, PALETTE.black);
  fillRect(grid, 71, 11, 3, 1, PALETTE.sparkleCyan);
  fillRect(grid, 71, 15, 3, 1, PALETTE.sparkleCyan);

  // Small z
  fillRect(grid, 76, 4, 4, 1, PALETTE.sparkleCyan);
  setPixel(grid, 78, 5, PALETTE.sparkleCyan);
  setPixel(grid, 77, 6, PALETTE.sparkleCyan);
  fillRect(grid, 76, 7, 4, 1, PALETTE.sparkleCyan);

  return gridToPixelSvg(grid);
}

function main() {
  const baseDir = path.join(__dirname, '..', 'src', 'assets', 'mascot');

  const dirs = ['coffee', 'brooming', 'bookshelf', 'thinking', 'bucket'];
  dirs.forEach(d => {
    const dirPath = path.join(baseDir, d);
    if (!fs.existsSync(dirPath)) {
      fs.mkdirSync(dirPath, { recursive: true });
    }
  });

  console.log('Generating Authentic 8-bit Coffee / Idle State (7 frames)...');
  const coffee = generateCoffeeState();
  coffee.forEach((svg, i) => {
    fs.writeFileSync(path.join(baseDir, 'coffee', `broomed-coffee-${i + 1}.svg`), svg);
  });

  console.log('Generating Authentic 8-bit Brooming / Working 1 State (7 frames)...');
  const brooming = generateBroomingState();
  brooming.forEach((svg, i) => {
    fs.writeFileSync(path.join(baseDir, 'brooming', `broomed-brooming-${i + 1}.svg`), svg);
  });

  console.log('Generating Authentic 8-bit Bookshelf / Working 2 State (7 frames)...');
  const bookshelf = generateBookshelfState();
  bookshelf.forEach((svg, i) => {
    fs.writeFileSync(path.join(baseDir, 'bookshelf', `broomed-bookshelf-${i + 1}.svg`), svg);
  });

  console.log('Generating Authentic 8-bit Chin Rub / Thinking State (7 frames)...');
  const thinking = generateThinkingState();
  thinking.forEach((svg, i) => {
    fs.writeFileSync(path.join(baseDir, 'thinking', `broomed-thinking-${i + 1}.svg`), svg);
  });

  console.log('Generating Authentic 8-bit Broom in Bucket Static Scene (1 SVG)...');
  const bucket = generateBucketState();
  fs.writeFileSync(path.join(baseDir, 'bucket', 'broomed-bucket.svg'), bucket);

  console.log('Done! All 5 authentic 8-bit / 16-bit pixel art states regenerated.');
}

main();
