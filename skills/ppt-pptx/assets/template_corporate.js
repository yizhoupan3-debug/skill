/**
 * template_corporate.js
 *
 * A professional corporate template variant for ppt-pptx.
 * Use for business presentations, quarterly reviews, strategy decks,
 * and client-facing materials.
 *
 * Usage: set `palette: corporate` in your outline YAML.
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
pptx.title = "Corporate Template";
pptx.lang = "zh-CN";

// ────────────────────────── Palette ──────────────────────────
const palette = {
  primary: "1B365D",         // navy blue — authority, trust
  primaryLight: "2A4A7F",    // lighter navy for hover/active
  accent: "D4AF37",          // muted gold — premium accent
  accentSoft: "FBF3DA",      // gold tint for highlight bg
  bg: "FFFFFF",              // white canvas
  surface: "F8F9FA",         // light gray surface
  surfaceDark: "1B365D",     // navy surface for header bars
  border: "E2E8F0",          // subtle gray border
  text: "1A202C",            // near-black text
  textSoft: "4A5568",        // secondary gray text
  textMute: "A0AEC0",        // muted caption text
  textOnDark: "FFFFFF",      // white text on dark surfaces
  success: "38A169",
  warning: "DD6B20",
  danger: "E53E3E",
};

pptx.defineSlideMaster({
  title: "OFFICECLI_SEMANTIC",
  background: { color: palette.bg },
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

/** Top header bar — branded navy strip */
function addHeaderBar(slide, title) {
  slide.addShape(pptx.ShapeType.rect, {
    x: 0, y: 0, w: 13.33, h: 0.72,
    fill: { color: palette.surfaceDark },
    line: { color: palette.surfaceDark, transparency: 100 },
  });
  slide.addText(title, {
    x: 0.6, y: 0.16, w: 7.0, h: 0.38,
    placeholder: "officecli_title",
    ...getTypography("h3", { color: palette.textOnDark, bold: true }),
  });
}

/** Bottom thin accent line */
function addFooterLine(slide, pageNum, total) {
  slide.addShape(pptx.ShapeType.rect, {
    x: 0, y: 7.38, w: 13.33, h: 0.12,
    fill: { color: palette.primary },
    line: { color: palette.primary, transparency: 100 },
  });
  if (pageNum) {
    slide.addText(`${pageNum} / ${total}`, {
      x: 12.0, y: 7.14, w: 1.0, h: 0.2,
      ...getTypography("caption", { color: palette.textMute, align: "right" }),
    });
  }
}

/** Corporate card with left accent bar */
function addCorpCard(slide, x, y, w, h, accentColor) {
  const ac = accentColor || palette.primary;
  slide.addShape(pptx.ShapeType.rect, {
    x, y, w, h,
    fill: { color: palette.bg },
    line: { color: palette.border, width: 0.5 },
    rectRadius: 0.04,
    shadow: safeOuterShadow({ blur: 4, offset: 1, color: "000000", transparency: 92 }),
  });
  // Left accent bar
  slide.addShape(pptx.ShapeType.rect, {
    x, y, w: 0.04, h,
    fill: { color: ac },
    line: { color: ac, transparency: 100 },
  });
}

/** Metric callout — large number with label */
function addMetric(slide, x, y, value, label) {
  slide.addText(value, {
    x, y, w: 2.4, h: 0.5,
    ...getSmartTypography("display", value, 2.4, 0.5, { color: palette.primary }),
  });
  slide.addText(label, {
    x, y: y + 0.52, w: 2.4, h: 0.16,
    ...getSmartTypography("body2", label, 2.4, 0.16, { color: palette.textSoft }),
  });
}

function sanitizeGeneratedDeck(fileName) {
  const script = path.resolve(process.cwd(), "scripts", "pptx_tool.js");
  if (!fs.existsSync(script)) return;
  const completed = spawnSync("node", [script, "sanitize-pptx", fileName], { stdio: "inherit" });
  if (completed.status !== 0) {
    throw new Error(`sanitize-pptx failed for ${fileName}`);
  }
}

// ────────────────────────── Demo Slides ──────────────────────────

// Cover Slide
const cover = pptx.addSlide({ masterName: "OFFICECLI_SEMANTIC" });
cover.background = { color: palette.primary };
cover.addShape(pptx.ShapeType.rect, {
  x: 0, y: 5.8, w: 13.33, h: 1.7,
  fill: { color: palette.primaryLight },
  line: { color: palette.primaryLight, transparency: 100 },
});
cover.addText("QUARTERLY BUSINESS REVIEW", {
  x: 0.9, y: 1.8, w: 8.0, h: 0.18,
  ...getTypography("overline", { color: palette.accent, charSpace: 2.0 }),
});
cover.addText("2026 Q1 Strategic\nPerformance Report", {
  x: 0.9, y: 2.3, w: 9.0, h: 1.0,
  placeholder: "officecli_title",
  ...getTypography("display", { color: palette.textOnDark, lineSpacing: 40 }),
});
cover.addText("Prepared by Strategy Team  |  March 2026", {
  x: 0.9, y: 3.5, w: 8.0, h: 0.2,
  ...getTypography("body1", { color: palette.textMute }),
});
cover.addShape(pptx.ShapeType.rect, {
  x: 0.9, y: 4.0, w: 0.6, h: 0.035,
  fill: { color: palette.accent },
  line: { color: palette.accent, transparency: 100 },
});
warnIfSlideHasOverlaps(cover, pptx);

// Metrics Slide
const s1 = pptx.addSlide({ masterName: "OFFICECLI_SEMANTIC" });
s1.background = { color: palette.bg };
addHeaderBar(s1, "Key Performance Indicators");

const metrics = [
  { value: "¥2.4B", label: "Total Revenue" },
  { value: "+18%", label: "YoY Growth" },
  { value: "92%", label: "Customer Retention" },
  { value: "4.6/5", label: "Satisfaction Score" },
];
metrics.forEach((m, i) => {
  const mx = 0.9 + i * 3.1;
  addCorpCard(s1, mx, 1.4, 2.8, 1.6);
  addMetric(s1, mx + 0.2, 1.7, m.value, m.label);
});

addFooterLine(s1, "02", "08");
warnIfSlideHasOverlaps(s1, pptx);
warnIfSlideElementsOutOfBounds(s1, pptx);

// Write
async function writeDeck() {
  await pptx.writeFile({ fileName: "deck_corporate.pptx" });
  sanitizeGeneratedDeck("deck_corporate.pptx");
  console.log("✅ Generated deck_corporate.pptx (corporate template demo)");
}

writeDeck().catch((error) => {
  console.error(error);
  process.exit(1);
});
