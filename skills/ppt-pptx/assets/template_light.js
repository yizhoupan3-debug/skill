/**
 * template_light.js
 * 
 * A clean, light/academic template variant for ppt-pptx.
 * Use this as the base for academic reports, corporate white-label,
 * or minimal day-mode presentations.
 * 
 * Usage: set `palette: light` in your outline YAML.
 */
"use strict";

const fs = require("fs");
const path = require("path");
const { spawnSync } = require("child_process");
const PptxGenJS = require("pptxgenjs");
const {
  imageSizingCrop,
  safeOuterShadow,
  warnIfSlideHasOverlaps,
  warnIfSlideElementsOutOfBounds,
  addStyledChart,
  getTypography,
  getSmartTypography,
} = require("./pptxgenjs_helpers");

const pptx = new PptxGenJS();
pptx.layout = "LAYOUT_WIDE";
pptx.title = "Light Academic Template";
pptx.lang = "zh-CN";
pptx.theme = {
  headFontFace: "Arial",
  bodyFontFace: "Arial",
  lang: "zh-CN",
};

// ────────────────────────── Palette ──────────────────────────
const palette = {
  stage: "FAFAFA",          // off-white background
  panel: "FFFFFF",          // pure white card
  panelSoft: "F5F5F3",      // light gray panel
  panelBorder: "E8E8E4",    // thin border between cards
  line: "E0E0DC",
  glow: "3A6CF4",           // academic blue accent
  glowSoft: "D6E3FF",       // light blue bg tint
  text: "1A1A1A",           // near-black body
  textSoft: "555550",       // secondary text
  textMute: "999990",       // tertiary / captions
  chip: "1A1A1A",
  chipText: "FFFFFF",
  accent: "3A6CF4",         // blue accent
  accentSoft: "EEF3FF",     // blue tint for highlight chips
};

pptx.defineSlideMaster({
  title: "OFFICECLI_SEMANTIC",
  background: { color: palette.stage },
  objects: [
    {
      placeholder: {
        options: {
          name: "officecli_title",
          type: "title",
        },
      },
    },
  ],
});

// ────────────────────────── Helpers ──────────────────────────
/** Top overline label (light mode: dark text on light bg) */
function addTopLabel(slide, text) {
  slide.addText(text, {
    x: 0.9, y: 0.36, w: 2.0, h: 0.12,
    ...getTypography("overline", { color: palette.textMute, charSpace: 1.4 }),
  });
}

/** Subtle bottom rule line instead of glow */
function addBottomRule(slide) {
  slide.addShape(pptx.ShapeType.line, {
    x: 0.86, y: 7.28, w: 11.6, h: 0,
    line: { color: palette.panelBorder, width: 0.75 },
  });
}

/** Light mode card panel — white card + border shadow */
function addLightPanel(slide, x, y, w, h) {
  slide.addShape(pptx.ShapeType.roundRect, {
    x, y, w, h,
    rectRadius: 0.06,
    line: { color: palette.panelBorder, width: 0.5 },
    fill: { color: palette.panel },
    shadow: {
      type: "outer",
      blur: 6,
      offset: 2,
      angle: 270,
      color: "000000",
      transparency: 90,
    },
  });
}

/** Section title for light template */
function addSectionTitle(slide, cn, en, x, y, w) {
  const cnW = Math.min(w * 0.72, 5.2);
  slide.addText(cn, { x, y, w: cnW, h: 0.28, placeholder: "officecli_title", ...getSmartTypography("h2", cn, cnW, 0.28, { color: palette.text }) });
  if (en) {
    slide.addText(en, { x, y: y + 0.38, w, h: 0.14, ...getSmartTypography("body2", en, w, 0.14, { color: palette.textMute, bold: true }) });
  }
  // Accent underline bar
  slide.addShape(pptx.ShapeType.line, {
    x, y: y + 0.30, w: 0.32, h: 0,
    line: { color: palette.accent, width: 2 },
  });
}

function sanitizeGeneratedDeck(fileName) {
  const script = path.resolve(process.cwd(), "sanitize_pptx.py");
  if (!fs.existsSync(script)) return;
  const completed = spawnSync("python3", [script, fileName], { stdio: "inherit" });
  if (completed.status !== 0) {
    throw new Error(`sanitize_pptx.py failed for ${fileName}`);
  }
}

// ────────────────────────── Slides ──────────────────────────
// Demo: Cover slide (light mode)
const cover = pptx.addSlide({ masterName: "OFFICECLI_SEMANTIC" });
cover.background = { color: "FFFFFF" };
cover.addShape(pptx.ShapeType.rect, {
  x: 0, y: 0, w: 0.06, h: 7.5,
  fill: { color: palette.accent }, line: { color: palette.accent, transparency: 100 },
});
cover.addText("RESEARCH REPORT", {
  x: 0.52, y: 1.62, w: 9.0, h: 0.16,
  ...getTypography("overline", { color: palette.textMute, charSpace: 2.0 }),
});
cover.addText("Academic Presentation\nTitle Goes Here", {
  x: 0.52, y: 2.1, w: 9.0, h: 0.8,
  placeholder: "officecli_title",
  ...getTypography("display", { color: palette.text, lineSpacing: 36 }),
});
cover.addText("Subtitle or institution — 2026", {
  x: 0.52, y: 3.0, w: 7.0, h: 0.18,
  ...getTypography("body1", { color: palette.textSoft }),
});
addBottomRule(cover);
cover.addText("01 / 08", { x: 12.2, y: 7.08, w: 0.8, h: 0.12, ...getTypography("caption", { color: palette.textMute, align: "right" }) });
warnIfSlideHasOverlaps(cover, pptx);

// Demo: Content slide — multi-card (light mode)
const slide0 = pptx.addSlide({ masterName: "OFFICECLI_SEMANTIC" });
slide0.background = { color: palette.stage };
addTopLabel(slide0, "SECTION 01");
addSectionTitle(slide0, "核心发现", "Key Findings", 0.9, 0.78, 5.0);

const cards = [
  { n: "01", text: "光色调背景更适合学术汇报和正式场合，给人以专业和可信的感觉。" },
  { n: "02", text: "蓝色主色调契合学术风格，使用 3A6CF4 作为精准蓝，清晰且不强势。" },
  { n: "03", text: "白色卡片 + 细框 + 轻阴影构成了轻量级的层次感，替代黑底的毛玻璃面板。" },
];
const cw = 3.56, gap = 0.22;
cards.forEach((c, i) => {
  const x = 0.94 + i * (cw + gap);
  addLightPanel(slide0, x, 2.0, cw, 3.8);
  slide0.addText(c.n, { x: x + 0.18, y: 2.28, w: 0.4, h: 0.2, ...getTypography("h3", { color: palette.accent }) });
  slide0.addText(c.text, { x: x + 0.18, y: 2.72, w: cw - 0.36, h: 2.88, ...getSmartTypography("body2", c.text, cw - 0.36, 2.88, { color: palette.textSoft, valign: "top", breakLine: true }) });
});
addBottomRule(slide0);
slide0.addText("02 / 08", { x: 12.2, y: 7.08, w: 0.8, h: 0.12, ...getTypography("caption", { color: palette.textMute, align: "right" }) });
warnIfSlideHasOverlaps(slide0, pptx);
warnIfSlideElementsOutOfBounds(slide0, pptx);

// Write
async function writeDeck() {
  await pptx.writeFile({ fileName: "deck_light.pptx" });
  sanitizeGeneratedDeck("deck_light.pptx");
  console.log("✅ Generated deck_light.pptx (light academic template demo)");
}

writeDeck().catch((error) => {
  console.error(error);
  process.exit(1);
});
