#!/usr/bin/env node
/**
 * outline_to_deck.js
 */
"use strict";

const fs = require("fs");
const path = require("path");

// Try to use our own helper locally to get the dominant color
let getDominantColor = async () => null;
let fixBulletOrphans = (arr) => arr;
let generateSpeakerNotes;
try {
  const helpers = require("../assets/pptxgenjs_helpers/index");
  if (helpers.getDominantColor) getDominantColor = helpers.getDominantColor;
  if (helpers.fixBulletOrphans) fixBulletOrphans = helpers.fixBulletOrphans;
  if (helpers.generateSpeakerNotes) generateSpeakerNotes = helpers.generateSpeakerNotes;
} catch (e) {
  // If run outside the expected structure, fallback to no-op
}

let yaml;
try {
  yaml = require("js-yaml");
} catch (_) {
  yaml = null;
}

// --- Palette definitions ---
const PALETTES = {
  dark: {
    stage: "000000",
    panel: "111111",
    panelSoft: "171717",
    line: "2A2A2A",
    glow: "7EA9FF",
    text: "F2F2EE",
    textSoft: "B9B9B2",
    textMute: "888883",
    chip: "F4F4EF",
    chipText: "111111",
  },
  light: {
    stage: "FAFAFA",
    panel: "FFFFFF",
    panelSoft: "F0F0F0",
    line: "E0E0E0",
    glow: "3B82F6",
    text: "1A1A1A",
    textSoft: "666666",
    textMute: "999999",
    chip: "1A1A1A",
    chipText: "FFFFFF",
  },
  academic: {
    stage: "F5F3EF",
    panel: "FFFFFF",
    panelSoft: "EDE9E3",
    line: "D4CFC7",
    glow: "2563EB",
    text: "1F2937",
    textSoft: "4B5563",
    textMute: "9CA3AF",
    chip: "1F2937",
    chipText: "FFFFFF",
  },
};

// --- Pattern detection ---
function detectPattern(slide) {
  if (slide.type === "cover") return "cover";
  if (slide.type === "closing") return "closing";
  if (slide.timeline && slide.timeline.length > 0) return "timeline";
  if (slide.steps && slide.steps.length > 0) return "process-flow";
  if (slide.comparison) return "comparison";
  if (slide.chart) return "data-panel";
  if (slide.metrics && slide.metrics.length >= 3) return "data-panel";
  if (slide.image && (!slide.bullets || slide.bullets.length <= 2)) return "hero-image";
  if (slide.image) return "image-text-split";
  if (slide.bullets && slide.bullets.length >= 3) return "multi-card";
  return "full-text";
}

// --- Auto-Pagination Engine ---
function reflowSlides(slides) {
  const newSlides = [];
  for (const slide of slides) {
    if (slide.type === "cover" || slide.type === "closing") {
      newSlides.push(slide);
      continue;
    }

    const pattern = detectPattern(slide);
    let titleText = slide.title || "";

    if (pattern === "multi-card" && slide.bullets && slide.bullets.length > 4) {
      const chunks = [];
      for (let i = 0; i < slide.bullets.length; i += 4) {
        chunks.push(slide.bullets.slice(i, i + 4));
      }
      chunks.forEach((chunk, idx) => {
        newSlides.push({ ...slide, title: `${titleText} (${idx + 1}/${chunks.length})`, bullets: chunk });
      });
      console.log(`♻️ Auto-reflow split multi-card slide '${titleText}' into ${chunks.length} pages.`);
    } else if (pattern === "process-flow" && slide.steps && slide.steps.length > 5) {
      const chunks = [];
      for (let i = 0; i < slide.steps.length; i += 4) {
        chunks.push(slide.steps.slice(i, i + 4));
      }
      chunks.forEach((chunk, idx) => {
        newSlides.push({ ...slide, title: `${titleText} (${idx + 1}/${chunks.length})`, steps: chunk });
      });
      console.log(`♻️ Auto-reflow split process-flow slide '${titleText}' into ${chunks.length} pages.`);
    } else if (pattern === "timeline" && slide.timeline && slide.timeline.length > 5) {
      const chunks = [];
      for (let i = 0; i < slide.timeline.length; i += 4) {
        chunks.push(slide.timeline.slice(i, i + 4));
      }
      chunks.forEach((chunk, idx) => {
        newSlides.push({ ...slide, title: `${titleText} (${idx + 1}/${chunks.length})`, timeline: chunk });
      });
      console.log(`♻️ Auto-reflow split timeline slide '${titleText}' into ${chunks.length} pages.`);
    } else if (pattern === "image-text-split" && slide.bullets) {
      const totalChars = slide.bullets.join("").length;
      if (totalChars > 150) {
        const mid = Math.ceil(slide.bullets.length / 2);
        newSlides.push({ ...slide, title: `${titleText} (1/2)`, bullets: slide.bullets.slice(0, mid) });
        newSlides.push({ ...slide, title: `${titleText} (2/2)`, bullets: slide.bullets.slice(mid) });
        console.log(`♻️ Auto-reflow split dense image-text-split slide '${titleText}' into 2 pages.`);
      } else {
        newSlides.push(slide);
      }
    } else {
      newSlides.push(slide);
    }
  }
  return newSlides;
}

// --- Code generation ---
function generatePreamble(outline, palette, dynamicGlow) {
  const p = PALETTES[palette] || PALETTES.dark;
  if (dynamicGlow) p.glow = dynamicGlow;

  return `const fs = require("fs");
const pptxgen = require("pptxgenjs");
const {
  imageSizingCrop,
  imageSizingContain,
  safeOuterShadow,
  warnIfSlideHasOverlaps,
  warnIfSlideElementsOutOfBounds,
  addStyledChart,
  addGlassPanel,
  getTypography,
  getSmartTypography,
} = require("./pptxgenjs_helpers");

const pptx = new pptxgen();
pptx.layout = "LAYOUT_WIDE";
pptx.title = ${JSON.stringify(outline.title || "Untitled Deck")};
pptx.lang = "zh-CN";

const palette = ${JSON.stringify(p, null, 2)};

// ── Reusable helpers ──
function addTopLabel(slide, text) {
  slide.addText(text, { x: 0.9, y: 0.38, w: 2.0, h: 0.12, ...getTypography("overline", { color: palette.textMute, charSpace: 1.2 }) });
}

function addBottomGlow(slide) {
  slide.addShape(pptx.ShapeType.rect, { x: 0.86, y: 6.86, w: 11.6, h: 0.018, line: { color: palette.glow, transparency: 100 }, fill: { color: palette.glow, transparency: 24 } });
}

function addMetricChip(slide, x, y, w, value, label, delay = 0) {
  addGlassPanel(slide, pptx, x, y, w, 0.94, { fill: palette.panelSoft, transparency: 15 });
  slide.addText(value, { x: x + 0.14, y: y + 0.18, w: w - 0.28, h: 0.18, ...getSmartTypography("metric", value, w - 0.28, 0.18, { color: palette.text, animate: { type: "fade", prop: "in", delay: delay + 0.1 } }) });
  slide.addText(label, { x: x + 0.14, y: y + 0.54, w: w - 0.28, h: 0.12, ...getSmartTypography("caption", label, w - 0.28, 0.12, { color: palette.textSoft, animate: { type: "fade", prop: "in", delay: delay + 0.2 } }) });
}

function addSectionTitle(slide, cn, en, x, y, w) {
  let cnW = Math.min(w * 0.62, 4.4);
  slide.addText(cn, { x, y, w: cnW, h: 0.24, ...getSmartTypography("h2", cn, cnW, 0.24, { color: palette.text, animate: { type: "fade", prop: "in" } }) });
  if (en) slide.addText(en, { x, y: y + 0.34, w, h: 0.14, ...getSmartTypography("body2", en, w, 0.14, { color: palette.textSoft, bold: true, animate: { type: "fade", prop: "in", delay: 0.1 } }) });
}

function finalizeSlide(slide, opts = {}) {
  if (!opts.skipOverlap) warnIfSlideHasOverlaps(slide, pptx, { ignoreDecorativeShapes: true });
  warnIfSlideElementsOutOfBounds(slide, pptx);
}

function fileExists(assetPath) {
  try {
    return !!assetPath && fs.existsSync(assetPath);
  } catch (_) {
    return false;
  }
}

function addOptionalImage(slide, assetPath, sizingFactory, fallback = {}) {
  if (fileExists(assetPath)) {
    slide.addImage({ path: assetPath, ...sizingFactory(assetPath) });
    return true;
  }

  slide.addShape(pptx.ShapeType.rect, {
    x: fallback.x ?? 0,
    y: fallback.y ?? 0,
    w: fallback.w ?? 13.333,
    h: fallback.h ?? 7.5,
    line: { color: fallback.fill || palette.panelSoft, transparency: 100 },
    fill: { color: fallback.fill || palette.panelSoft, transparency: fallback.transparency ?? 0 },
  });

  if (fallback.label) {
    slide.addText(fallback.label, {
      x: (fallback.x ?? 0) + 0.18,
      y: (fallback.y ?? 0) + 0.18,
      w: Math.max((fallback.w ?? 4) - 0.36, 1.2),
      h: 0.14,
      ...getTypography("caption", { color: palette.textMute }),
    });
  }

  return false;
}

const totalSlides = ${outline.totalSlides || (outline.slides || []).length + 2};
`;
}

function generateCover(outline) {
  const coverImage = outline.coverImage || "./assets/cover.jpg";
  return `
// ── Cover ──
const cover = pptx.addSlide();
cover.background = { color: palette.stage };
addOptionalImage(cover, ${JSON.stringify(coverImage)}, (assetPath) => imageSizingCrop(assetPath, 0, 0, 13.333, 7.5), {
  x: 0, y: 0, w: 13.333, h: 7.5, fill: palette.panelSoft, transparency: 0, label: "COVER IMAGE OPTIONAL"
});
cover.addShape(pptx.ShapeType.rect, { x: 0, y: 0, w: 13.333, h: 7.5, line: { color: palette.stage, transparency: 100 }, fill: { color: palette.stage, transparency: 40 } });
cover.addShape(pptx.ShapeType.rect, { x: 0, y: 0, w: 6.1, h: 7.5, line: { color: palette.stage, transparency: 100 }, fill: { color: palette.stage, transparency: 22 } });
addTopLabel(cover, "PRESENTATION");
cover.addText(${JSON.stringify(outline.title || "Title")}, { x: 0.92, y: 1.76, w: 4.64, h: 1.06, ...getTypography("display", { color: palette.text, animate: { type: "fade", prop: "in", delay: 0.1 } }) });
${outline.subtitle ? `cover.addText(${JSON.stringify(outline.subtitle)}, { x: 0.96, y: 3.02, w: 4.48, h: 0.66, ...getTypography("body1", { color: palette.textSoft, animate: { type: "fade", prop: "in", delay: 0.3 } }) });` : ""}
${outline.presenter || outline.date ? `cover.addText(${JSON.stringify([outline.presenter, outline.date].filter(Boolean).join(" / "))}, { x: 0.96, y: 4.48, w: 3.0, h: 0.14, ...getTypography("body2", { color: palette.textSoft, animate: { type: "fade", prop: "in", delay: 0.5 } }) });` : ""}
cover.addText("01 / " + String(totalSlides).padStart(2, "0"), { x: 12.2, y: 7.03, w: 0.4, h: 0.12, ...getTypography("caption", { color: palette.textMute, align: "right" }) });
addBottomGlow(cover);
finalizeSlide(cover, { skipOverlap: true });
`;
}

function generateContentSlide(slide, index, sectionNum) {
  const pattern = detectPattern(slide);
  const pageNum = '"' + String(index + 2).padStart(2, "0") + ' / " + String(totalSlides).padStart(2, "0")';

  // -- Pre-process bullets: fix orphan / widow lines --
  const cleanSlide = { ...slide };
  if (cleanSlide.bullets && Array.isArray(cleanSlide.bullets)) {
    const cardW = cleanSlide.bullets.length <= 2 ? 5.48 : cleanSlide.bullets.length === 3 ? 3.56 : 2.62;
    const textW = pattern === "full-text" ? 10.9 : pattern === "image-text-split" ? 5.58 : cardW - 0.36;
    cleanSlide.bullets = fixBulletOrphans(cleanSlide.bullets, textW, 11.5);
  }

  // -- Generate speaker notes --
  const totalCount = '" + String(totalSlides) + "';
  const speakerNotesCode = generateSpeakerNotes
    ? `slide${index}.addNotes(${JSON.stringify(generateSpeakerNotes(slide, pattern, index + 2, parseInt("${totalCount}") || 10))});`
    : "";

  let code = `
// ── Slide ${index + 2}: ${slide.title || "Untitled"} (${pattern}) ──
const slide${index} = pptx.addSlide();
slide${index}.background = { color: palette.stage };
addTopLabel(slide${index}, "SECTION ${String(sectionNum).padStart(2, "0")}");
addSectionTitle(slide${index}, ${JSON.stringify(cleanSlide.title || "")}, ${JSON.stringify(cleanSlide.subtitle || "")}, 0.92, 0.96, 5.0);
`;

  switch (pattern) {
    case "multi-card":
      code += generateMultiCard(cleanSlide, index);
      break;
    case "data-panel":
      code += generateDataPanel(cleanSlide, index);
      break;
    case "comparison":
      code += generateComparison(cleanSlide, index);
      break;
    case "image-text-split":
      code += generateImageTextSplit(cleanSlide, index);
      break;
    case "hero-image":
      code += generateHeroImage(cleanSlide, index);
      break;
    case "timeline":
      code += generateTimeline(cleanSlide, index);
      break;
    case "process-flow":
      code += generateProcessFlow(cleanSlide, index);
      break;
    default:
      code += generateFullText(cleanSlide, index);
  }

  code += `
slide${index}.addText(${pageNum}, { x: 12.2, y: 7.03, w: 0.4, h: 0.12, ...getTypography("caption", { color: palette.textMute, align: "right" }) });
addBottomGlow(slide${index});
finalizeSlide(slide${index});
${speakerNotesCode}
`;
  return code;
}

function generateMultiCard(slide, idx) {
  const bullets = slide.bullets || [];
  const cardCount = Math.min(bullets.length, 4);
  const cardW = cardCount <= 2 ? 5.48 : cardCount === 3 ? 3.56 : 2.62;
  const gap = 0.22;
  let code = "";
  for (let i = 0; i < cardCount; i++) {
    const x = 0.94 + i * (cardW + gap);
    const delay = 0.2 + (i * 0.2);
    code += `addGlassPanel(slide${idx}, pptx, ${x.toFixed(2)}, 2.0, ${cardW}, 3.8, { fill: palette.panelSoft, transparency: 10 });
slide${idx}.addText(${JSON.stringify(String(i + 1).padStart(2, "0"))}, { x: ${(x + 0.18).toFixed(2)}, y: 2.28, w: 0.4, h: 0.2, ...getTypography("h3", { color: palette.text, animate: { type: "fade", prop: "in", delay: ${delay} } }) });
slide${idx}.addText(${JSON.stringify(bullets[i] || "")}, { x: ${(x + 0.18).toFixed(2)}, y: 2.72, w: ${(cardW - 0.36).toFixed(2)}, h: 2.8, ...getSmartTypography("body2", ${JSON.stringify(bullets[i] || "")}, ${(cardW - 0.36).toFixed(2)}, 2.8, { color: palette.textSoft, valign: "top", breakLine: true, animate: { type: "fade", prop: "in", delay: ${delay + 0.1} } }) });
`;
  }
  return code;
}

function generateDataPanel(slide, idx) {
  const metrics = slide.metrics || [];
  let code = "";
  const chipCount = Math.min(metrics.length, 5);
  const chipW = chipCount > 0 ? Math.min(2.2, (11.4 / chipCount) - 0.2) : 2.0;
  for (let i = 0; i < chipCount; i++) {
    const x = 0.94 + i * (chipW + 0.22);
    const delay = 0.2 + (i * 0.15);
    code += `addMetricChip(slide${idx}, ${x.toFixed(2)}, 2.3, ${chipW.toFixed(2)}, ${JSON.stringify(metrics[i].value || "")}, ${JSON.stringify(metrics[i].label || "")}, ${delay});\n`;
  }
  code += `addGlassPanel(slide${idx}, pptx, 0.94, 3.56, 11.42, 2.24, { fill: palette.panelSoft, transparency: 8 });\n`;
  if (slide.chart) {
    code += `addStyledChart(slide${idx}, pptx, ${JSON.stringify(slide.chart.type || "bar")}, {
  series: ${JSON.stringify(slide.chart.series || [])},
  categories: ${JSON.stringify(slide.chart.categories || [])},
  position: { x: 1.1, y: 3.7, w: 11.1, h: 1.96 },
});\n`;
  }
  return code;
}

function generateComparison(slide, idx) {
  const left = slide.comparison?.left || {};
  const right = slide.comparison?.right || {};
  const leftString = (left.items || []).map((s, i) => (i + 1) + ". " + s).join("\\n");
  const rightString = (right.items || []).map((s, i) => (i + 1) + ". " + s).join("\\n");
  return `addGlassPanel(slide${idx}, pptx, 0.94, 1.9, 5.48, 4.4, { fill: palette.panelSoft, transparency: 10 });
addGlassPanel(slide${idx}, pptx, 6.72, 1.9, 5.48, 4.4, { fill: palette.panelSoft, transparency: 10 });
slide${idx}.addText(${JSON.stringify(left.title || "A")}, { x: 1.18, y: 2.18, w: 1.1, h: 0.14, ...getSmartTypography("body1", ${JSON.stringify(left.title || "A")}, 1.1, 0.14, { color: palette.text, bold: true, animate: { type: "fade", prop: "in", delay: 0.2 } }) });
slide${idx}.addText(${JSON.stringify(leftString)}, { x: 1.18, y: 2.54, w: 4.82, h: 3.2, ...getSmartTypography("body2", ${JSON.stringify(leftString)}, 4.82, 3.2, { color: palette.textSoft, valign: "top", breakLine: true, animate: { type: "fade", prop: "in", delay: 0.3 } }) });
slide${idx}.addText(${JSON.stringify(right.title || "B")}, { x: 6.96, y: 2.18, w: 1.1, h: 0.14, ...getSmartTypography("body1", ${JSON.stringify(right.title || "B")}, 1.1, 0.14, { color: palette.text, bold: true, animate: { type: "fade", prop: "in", delay: 0.4 } }) });
slide${idx}.addText(${JSON.stringify(rightString)}, { x: 6.96, y: 2.54, w: 4.82, h: 3.2, ...getSmartTypography("body2", ${JSON.stringify(rightString)}, 4.82, 3.2, { color: palette.textSoft, valign: "top", breakLine: true, animate: { type: "fade", prop: "in", delay: 0.5 } }) });
`;
}

function generateImageTextSplit(slide, idx) {
  const imagePath = slide.image || "./assets/placeholder.jpg";
  const bullets = slide.bullets || [];
  const textStr = bullets.map((s, i) => (i + 1) + ". " + s).join("\\n");
  return `addGlassPanel(slide${idx}, pptx, 0.94, 1.76, 5.14, 4.44, { fill: palette.panelSoft, transparency: 15 });
const slide${idx}HasImage = addOptionalImage(slide${idx}, ${JSON.stringify(imagePath)}, (assetPath) => imageSizingCrop(assetPath, 1.0, 1.82, 5.02, 4.32), {
  x: 1.0, y: 1.82, w: 5.02, h: 4.32, fill: palette.panel, transparency: 0, label: "OPTIONAL IMAGE"
});
if (slide${idx}HasImage) {
  slide${idx}.addShape(pptx.ShapeType.roundRect, { x: 1.0, y: 1.82, w: 5.02, h: 4.32, rectRadius: 0.06, line: { color: palette.stage, transparency: 100 }, fill: { color: palette.stage, transparency: 26 } });
}
${bullets.length > 0 ? `slide${idx}.addText(${JSON.stringify(textStr)}, { x: 6.56, y: 2.0, w: 5.58, h: 4.0, ...getSmartTypography("body1", ${JSON.stringify(textStr)}, 5.58, 4.0, { color: palette.textSoft, valign: "top", breakLine: true, animate: { type: "fade", prop: "in", delay: 0.3 } }) });` : ""}
`;
}

function generateHeroImage(slide, idx) {
  const imagePath = slide.image || "./assets/placeholder.jpg";
  return `const slide${idx}HasHero = addOptionalImage(slide${idx}, ${JSON.stringify(imagePath)}, (assetPath) => imageSizingCrop(assetPath, 0, 1.4, 13.333, 6.1), {
  x: 0, y: 1.4, w: 13.333, h: 6.1, fill: palette.panelSoft, transparency: 0, label: "OPTIONAL IMAGE"
});
if (slide${idx}HasHero) {
  slide${idx}.addShape(pptx.ShapeType.rect, { x: 0, y: 1.4, w: 13.333, h: 6.1, line: { color: palette.stage, transparency: 100 }, fill: { color: palette.stage, transparency: 40 } });
}
${slide.caption ? `slide${idx}.addText(${JSON.stringify(slide.caption)}, { x: 0.96, y: 5.8, w: 6.0, h: 0.36, ...getTypography("body1", { color: palette.textSoft, animate: { type: "fade", prop: "in", delay: 0.2 } }) });` : ""}
`;
}

function generateFullText(slide, idx) {
  const bullets = slide.bullets || [];
  if (bullets.length > 0) {
    const textStr = bullets.map((s, i) => (i + 1) + ". " + s).join("\\n");
    return `addGlassPanel(slide${idx}, pptx, 0.94, 1.76, 11.42, 4.44, { fill: palette.panelSoft, transparency: 10 });
slide${idx}.addText(${JSON.stringify(textStr)}, { x: 1.18, y: 2.04, w: 10.9, h: 3.88, ...getSmartTypography("body1", ${JSON.stringify(textStr)}, 10.9, 3.88, { color: palette.textSoft, valign: "top", breakLine: true, animate: { type: "fade", prop: "in", delay: 0.2 } }) });
`;
  }
  return `addGlassPanel(slide${idx}, pptx, 0.94, 1.76, 11.42, 4.44, { fill: palette.panelSoft, transparency: 10 });
`;
}

function generateClosing(outline) {
  const coverImage = outline.coverImage || "./assets/cover.jpg";
  return `
// ── Closing ──
const closing = pptx.addSlide();
closing.background = { color: palette.stage };
addOptionalImage(closing, ${JSON.stringify(coverImage)}, (assetPath) => imageSizingCrop(assetPath, 0, 0, 13.333, 7.5), {
  x: 0, y: 0, w: 13.333, h: 7.5, fill: palette.panelSoft, transparency: 0, label: "COVER IMAGE OPTIONAL"
});
closing.addShape(pptx.ShapeType.rect, { x: 0, y: 0, w: 13.333, h: 7.5, line: { color: palette.stage, transparency: 100 }, fill: { color: palette.stage, transparency: 52 } });
addTopLabel(closing, "FINAL SLIDE");
closing.addText("THANK YOU", { x: 4.18, y: 2.1, w: 4.98, h: 0.42, ...getTypography("display", { color: palette.text, align: "center", animate: { type: "fade", prop: "in", delay: 0.2 } }) });
closing.addText("" + String(totalSlides).padStart(2, "0") + " / " + String(totalSlides).padStart(2, "0"), { x: 12.2, y: 7.03, w: 0.4, h: 0.12, ...getTypography("caption", { color: palette.textMute, align: "right" }) });
addBottomGlow(closing);
finalizeSlide(closing, { skipOverlap: true });
`;
}

// --- Main ---
async function main() {
  const args = process.argv.slice(2);
  if (args.includes("--help") || args.includes("-h")) {
    console.error("Usage: node outline_to_deck.js <outline.yaml|outline.json> [-o output.js]");
    process.exit(0);
  }
  if (args.length === 0) {
    console.error("Usage: node outline_to_deck.js <outline.yaml|outline.json> [-o output.js]");
    process.exit(1);
  }

  const inputFile = args[0];
  const outputIndex = args.indexOf("-o");
  const outputFile = outputIndex >= 0 && args[outputIndex + 1] ? args[outputIndex + 1] : "deck.js";

  const raw = fs.readFileSync(inputFile, "utf8");
  let outline;

  if (inputFile.endsWith(".json")) {
    outline = JSON.parse(raw);
  } else if (yaml) {
    outline = yaml.load(raw);
  } else {
    console.error("js-yaml is required for YAML input. Install with: npm install js-yaml");
    process.exit(1);
  }

  const paletteName = outline.palette || "dark";
  
  // Direction 3: Extract ambient color from cover image dynamically
  let dynamicGlow = null;
  const coverPath = outline.coverImage || path.join(path.dirname(inputFile), "assets/cover.jpg");
  if (fs.existsSync(coverPath)) {
    dynamicGlow = await getDominantColor(coverPath);
  }

  const slides = reflowSlides(outline.slides || []);
  outline.totalSlides = slides.length + 2;

  let code = generatePreamble(outline, paletteName, dynamicGlow);
  code += generateCover(outline);

  let sectionNum = 1;
  for (let i = 0; i < slides.length; i++) {
    const slide = slides[i];
    if (slide.type === "cover" || slide.type === "closing") continue;
    code += generateContentSlide(slide, i, sectionNum);
    sectionNum++;
  }

  code += generateClosing(outline);
  code += `\npptx.writeFile({ fileName: "deck.pptx" });\n`;

  fs.writeFileSync(outputFile, code, "utf8");
  console.log(`✅ Generated ${outputFile} with ${slides.length + 2} slides (cover + ${slides.length} content + closing)`);
  if (dynamicGlow) {
    console.log(`✅ Ambient color ${dynamicGlow} extracted and applied to glows.`);
  }
}

main();
