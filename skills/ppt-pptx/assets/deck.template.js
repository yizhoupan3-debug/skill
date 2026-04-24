const fs = require("fs");
const path = require("path");
const { spawnSync } = require("child_process");
const pptxgen = require("pptxgenjs");
const {
  imageSizingCrop,
  imageSizingContain,
  safeOuterShadow,
  warnIfSlideHasOverlaps,
  warnIfSlideElementsOutOfBounds,
  addGlassPanel,
  getTypography,
} = require("./pptxgenjs_helpers");

const pptx = new pptxgen();
pptx.layout = "LAYOUT_WIDE";
pptx.author = "Codex";
pptx.company = "OpenAI";
pptx.subject = "Black Luxury PPTX";
pptx.title = "Black Luxury Deck";
pptx.lang = "zh-CN";
pptx.theme = {
  headFontFace: "Arial",
  bodyFontFace: "Arial",
  lang: "zh-CN",
};

const palette = {
  stage: "000000",
  stageSoft: "090909",
  panel: "111111",
  panelSoft: "171717",
  line: "2A2A2A",
  glow: "7EA9FF",
  white: "F6F6F1",
  text: "F2F2EE",
  textSoft: "B9B9B2",
  textMute: "888883",
  chip: "F4F4EF",
  chipText: "111111",
};

// Use a deliberately softened / blurred export for the cover background.
const assets = {
  coverBlur: "./assets/cover-blur.jpg",
  coverFocus: "./assets/cover-focus.jpg",
  image1: "./assets/content-image-1.jpg",
  image2: "./assets/content-image-2.jpg",
  diagram: "./assets/diagram.png",
};

pptx.defineSlideMaster({
  title: "BLACK_LUXURY",
  background: { color: palette.stage },
  objects: [
    {
      rect: {
        x: 0,
        y: 0,
        w: 13.333,
        h: 7.5,
        line: { color: palette.stage, transparency: 100 },
        fill: { color: palette.stage },
      },
    },
    {
      placeholder: {
        options: {
          name: "officecli_title",
          type: "title",
        },
      },
    },
  ],
  slideNumber: {
    x: 12.2,
    y: 7.03,
    w: 0.4,
    h: 0.12,
    fontFace: "Arial",
    fontSize: 8,
    color: palette.textMute,
    align: "right",
  },
});

function addTopLabel(slide, text = "BLACK LUXURY PPT") {
  slide.addText(text, {
    x: 0.9,
    y: 0.38,
    w: 2.0,
    h: 0.12,
    ...getTypography("overline", { color: palette.textMute, charSpace: 1.2 }),
  });
}

function addBottomGlow(slide) {
  slide.addShape(pptx.ShapeType.rect, {
    x: 0.86,
    y: 6.86,
    w: 11.6,
    h: 0.018,
    line: { color: palette.glow, transparency: 100 },
    fill: { color: palette.glow, transparency: 24 },
  });
}

function addDarkPanel(slide, x, y, w, h, options = {}) {
  slide.addShape(pptx.ShapeType.roundRect, {
    x,
    y,
    w,
    h,
    rectRadius: options.radius ?? 0.08,
    line: {
      color: options.lineColor || palette.line,
      pt: options.linePt || 0.7,
      transparency: options.lineTransparency ?? 0,
    },
    fill: {
      color: options.fill || palette.panel,
      transparency: options.fillTransparency ?? 0,
    },
    shadow: safeOuterShadow("000000", options.shadowOpacity ?? 0.16, 45, 1.8, 0.9),
  });
}

function addImageCard(slide, options) {
  addDarkPanel(slide, options.x, options.y, options.w, options.h, {
    fill: options.fill || palette.panel,
    lineColor: options.lineColor || palette.line,
    lineTransparency: options.lineTransparency ?? 0,
  });

  if (options.path && fs.existsSync(options.path)) {
    slide.addImage({
      path: options.path,
      ...((options.mode || "crop") === "contain"
        ? imageSizingContain(options.path, options.x + 0.06, options.y + 0.06, options.w - 0.12, options.h - 0.12)
        : imageSizingCrop(options.path, options.x + 0.06, options.y + 0.06, options.w - 0.12, options.h - 0.12)),
    });
  } else {
    slide.addShape(pptx.ShapeType.roundRect, {
      x: options.x + 0.06,
      y: options.y + 0.06,
      w: options.w - 0.12,
      h: options.h - 0.12,
      rectRadius: Math.max((options.radius ?? 0.08) - 0.02, 0.04),
      line: { color: palette.line, transparency: 0, pt: 0.5 },
      fill: { color: palette.panelSoft, transparency: 0 },
    });
    slide.addText(options.missingLabel || "OPTIONAL IMAGE", {
      x: options.x + 0.22,
      y: options.y + 0.22,
      w: options.w - 0.44,
      h: 0.14,
      fontFace: "Arial",
      fontSize: 8.2,
      color: palette.textMute,
      margin: 0,
    });
    return;
  }

  if (options.overlay) {
    slide.addShape(pptx.ShapeType.roundRect, {
      x: options.x + 0.06,
      y: options.y + 0.06,
      w: options.w - 0.12,
      h: options.h - 0.12,
      rectRadius: Math.max((options.radius ?? 0.08) - 0.02, 0.04),
      line: { color: options.overlay, transparency: 100 },
      fill: { color: options.overlay, transparency: options.overlayTransparency ?? 42 },
    });
  }

  if (options.label) {
    slide.addShape(pptx.ShapeType.roundRect, {
      x: options.x + 0.18,
      y: options.y + 0.18,
      w: options.labelW || 0.88,
      h: 0.27,
      rectRadius: 0.05,
      line: { color: palette.chip, transparency: 100 },
      fill: { color: palette.chip, transparency: 0 },
    });
    slide.addText(options.label, {
      x: options.x + 0.24,
      y: options.y + 0.23,
      w: (options.labelW || 0.88) - 0.12,
      h: 0.1,
      fontFace: "Arial",
      fontSize: 8.2,
      color: palette.chipText,
      bold: true,
      align: "center",
      margin: 0,
    });
  }
}

function addMetricChip(slide, x, y, w, value, label) {
  slide.addShape(pptx.ShapeType.roundRect, {
    x,
    y,
    w,
    h: 0.94,
    rectRadius: 0.06,
    line: { color: palette.line, pt: 0.6, transparency: 0 },
    fill: { color: palette.panelSoft },
  });
  slide.addText(value, {
    x: x + 0.14,
    y: y + 0.18,
    w: w - 0.28,
    h: 0.18,
    fontFace: "Arial",
    fontSize: 17,
    color: palette.text,
    bold: true,
    margin: 0,
  });
  slide.addText(label, {
    x: x + 0.14,
    y: y + 0.54,
    w: w - 0.28,
    h: 0.12,
    fontFace: "Arial",
    fontSize: 8.8,
    color: palette.textSoft,
    margin: 0,
  });
}

function addSectionTitle(slide, cn, en, x, y, w) {
  slide.addText(cn, {
    x,
    y,
    w: Math.min(w * 0.62, 4.4),
    h: 0.24,
    placeholder: "officecli_title",
    ...getTypography("h2", { color: palette.text, animate: { type: "fade", prop: "in" } }),
  });
  slide.addText(en, {
    x: x,
    y: y + 0.34,
    w: w,
    h: 0.14,
    ...getTypography("body2", { color: palette.textSoft, bold: true, animate: { type: "fade", prop: "in", delay: 0.1 } }),
  });
}

function finalizeSlide(slide, options = {}) {
  if (!options.skipOverlap) {
    warnIfSlideHasOverlaps(slide, pptx);
  }
  warnIfSlideElementsOutOfBounds(slide, pptx);
}

function sanitizeGeneratedDeck(fileName) {
  const tool = process.env.PPT_PPTX_RUST_TOOL_BIN || "ppt";
  const completed = spawnSync(tool, ["sanitize-pptx", fileName], { stdio: "inherit" });
  if (completed.status !== 0) {
    throw new Error(`sanitize-pptx failed for ${fileName}`);
  }
}

const cover = pptx.addSlide({ masterName: "BLACK_LUXURY" });
if (fs.existsSync(assets.coverBlur)) {
  cover.addImage({
    path: assets.coverBlur,
    ...imageSizingCrop(assets.coverBlur, 0, 0, 13.333, 7.5),
  });
}
cover.addShape(pptx.ShapeType.rect, {
  x: 0,
  y: 0,
  w: 13.333,
  h: 7.5,
  line: { color: palette.stage, transparency: 100 },
  fill: { color: palette.stage, transparency: 40 },
});
cover.addShape(pptx.ShapeType.rect, {
  x: 0,
  y: 0,
  w: 6.1,
  h: 7.5,
  line: { color: palette.stage, transparency: 100 },
  fill: { color: palette.stage, transparency: 22 },
});
addTopLabel(cover, "BLACK LUXURY PPT");
cover.addText("课程汇报标题", {
  x: 0.92,
  y: 1.76,
  w: 4.64,
  h: 1.06,
  placeholder: "officecli_title",
  fontFace: "Arial",
  fontSize: 28,
  color: palette.text,
  bold: true,
  breakLine: true,
  margin: 0,
});
cover.addText("首页用一张经过虚化处理的大背景图，信息层落在清晰的暗色保护区里，而不是直接压在嘈杂图片上。", {
  x: 0.96,
  y: 3.02,
  w: 4.48,
  h: 0.66,
  fontFace: "Arial",
  fontSize: 11.2,
  color: palette.textSoft,
  margin: 0,
});
cover.addShape(pptx.ShapeType.roundRect, {
  x: 0.96,
  y: 4.24,
  w: 3.28,
  h: 0.96,
  rectRadius: 0.06,
  line: { color: palette.line, pt: 0.6, transparency: 0 },
  fill: { color: palette.panel, transparency: 8 },
});
cover.addText("报告人 / 团队", {
  x: 1.14,
  y: 4.48,
  w: 1.2,
  h: 0.12,
  fontFace: "Arial",
  fontSize: 8.8,
  color: palette.textMute,
  bold: true,
  margin: 0,
});
cover.addText("课程 / 日期 / 项目", {
  x: 1.14,
  y: 4.78,
  w: 1.9,
  h: 0.12,
  fontFace: "Arial",
  fontSize: 10,
  color: palette.text,
  margin: 0,
});
addImageCard(cover, {
  path: assets.coverFocus,
  x: 8.72,
  y: 4.76,
  w: 3.22,
  h: 1.54,
  overlay: palette.stage,
  overlayTransparency: 28,
  label: "FOCUS",
  labelW: 0.78,
});
addBottomGlow(cover);
finalizeSlide(cover, { skipOverlap: true });

const intro = pptx.addSlide({ masterName: "BLACK_LUXURY" });
addTopLabel(intro, "SECTION 01");
addSectionTitle(intro, "问题概况", "Project framing", 0.92, 0.96, 5.0);
intro.addText("黑底 deck 不是靠黑就高级，而是靠更少的元素、更强的对比和更稳的保护层。", {
  x: 0.94,
  y: 1.76,
  w: 4.58,
  h: 0.36,
  fontFace: "Arial",
  fontSize: 10.8,
  color: palette.textSoft,
  margin: 0,
});
addMetricChip(intro, 0.94, 2.3, 1.86, "01", "主问题");
addMetricChip(intro, 3.0, 2.3, 1.86, "03", "核心证据");
addMetricChip(intro, 5.06, 2.3, 1.86, "2-4", "信息层级");
addDarkPanel(intro, 0.94, 3.56, 5.98, 2.22, {
  fill: palette.panelSoft,
});
intro.addText("信息要点", {
  x: 1.14,
  y: 3.84,
  w: 1.2,
  h: 0.14,
  fontFace: "Arial",
  fontSize: 11.4,
  color: palette.text,
  bold: true,
  margin: 0,
});
intro.addText(
  "1. 先给一句判断，再给证据。\n2. 每页只保留 2-4 个信息区。\n3. 标题、图像、注释必须有明确音量差。",
  {
    x: 1.14,
    y: 4.18,
    w: 5.3,
    h: 0.88,
    fontFace: "Arial",
    fontSize: 10.6,
    color: palette.textSoft,
    margin: 0,
    breakLine: true,
  }
);
addImageCard(intro, {
  path: assets.image1,
  x: 7.22,
  y: 1.08,
  w: 5.14,
  h: 4.74,
  overlay: palette.stage,
  overlayTransparency: 26,
  label: "IMAGE",
});
addBottomGlow(intro);
finalizeSlide(intro);

const data = pptx.addSlide({ masterName: "BLACK_LUXURY" });
addTopLabel(data, "SECTION 02");
addSectionTitle(data, "证据面板", "Evidence board", 0.92, 0.96, 5.0);
data.addText("图像和数据应该被压进同一块暗色面板里看，而不是散成满页小组件。", {
  x: 0.94,
  y: 1.76,
  w: 5.04,
  h: 0.36,
  fontFace: "Arial",
  fontSize: 10.8,
  color: palette.textSoft,
  margin: 0,
});
addMetricChip(data, 0.94, 2.3, 2.12, "21.8°C", "年均温");
addMetricChip(data, 3.28, 2.3, 2.12, "77%", "相对湿度");
addMetricChip(data, 5.62, 2.3, 2.12, "+8°C", "热岛峰值");
addMetricChip(data, 7.96, 2.3, 2.12, "-4.73°C", "增绿潜力");
addDarkPanel(data, 0.94, 3.56, 11.42, 2.24, { fill: palette.panelSoft });
addImageCard(data, {
  path: assets.diagram,
  x: 7.18,
  y: 3.78,
  w: 4.94,
  h: 1.8,
  mode: "contain",
  overlay: palette.stage,
  overlayTransparency: 18,
});
data.addText("读图结论", {
  x: 1.18,
  y: 3.86,
  w: 1.2,
  h: 0.14,
  fontFace: "Arial",
  fontSize: 11.4,
  color: palette.text,
  bold: true,
  margin: 0,
});
data.addText(
  "先认清气候约束，再谈空间形式。\n如果背景是湿热城市，无遮阴硬地本身就是高成本设计。\n信息越复杂，越要先给一句结论。",
  {
    x: 1.18,
    y: 4.18,
    w: 5.28,
    h: 0.92,
    fontFace: "Arial",
    fontSize: 10.4,
    color: palette.textSoft,
    margin: 0,
    breakLine: true,
  }
);
addBottomGlow(data);
finalizeSlide(data);

const compare = pptx.addSlide({ masterName: "BLACK_LUXURY" });
addTopLabel(compare, "SECTION 03");
addSectionTitle(compare, "设计对照", "Comparison layout", 0.92, 0.96, 5.0);
addGlassPanel(compare, pptx, 0.94, 1.9, 5.48, 4.4, { fill: palette.panelSoft, transparency: 15 });
addGlassPanel(compare, pptx, 6.72, 1.9, 5.48, 4.4, { fill: palette.panelSoft, transparency: 15 });
compare.addText("现状问题", {
  x: 1.18,
  y: 2.18,
  w: 1.1,
  h: 0.14,
  fontFace: "Arial",
  fontSize: 11.4,
  color: palette.text,
  bold: true,
  margin: 0,
});
compare.addText(
  "1. 白天停留成本高。\n2. 关键文字常被图片抢走。\n3. 组件太多会让人抓不到重点。",
  {
    x: 1.18,
    y: 2.54,
    w: 4.82,
    h: 1.16,
    fontFace: "Arial",
    fontSize: 10.6,
    color: palette.textSoft,
    margin: 0,
    breakLine: true,
  }
);
addImageCard(compare, {
  path: assets.image2,
  x: 1.12,
  y: 4.16,
  w: 5.12,
  h: 1.92,
  overlay: palette.stage,
  overlayTransparency: 20,
  label: "CURRENT",
});
compare.addText("优化方向", {
  x: 6.96,
  y: 2.18,
  w: 1.1,
  h: 0.14,
  fontFace: "Arial",
  fontSize: 11.4,
  color: palette.text,
  bold: true,
  margin: 0,
});
compare.addText(
  "1. 封面必须有虚化背景大图。\n2. 文字必须落在深色保护层里。\n3. 每页只保留少量高价值模块。",
  {
    x: 6.96,
    y: 2.54,
    w: 4.82,
    h: 1.16,
    fontFace: "Arial",
    fontSize: 10.6,
    color: palette.textSoft,
    margin: 0,
    breakLine: true,
  }
);
addImageCard(compare, {
  path: assets.coverFocus,
  x: 6.9,
  y: 4.16,
  w: 5.12,
  h: 1.92,
  overlay: palette.stage,
  overlayTransparency: 28,
  label: "TARGET",
});
addBottomGlow(compare);
finalizeSlide(compare);

const closing = pptx.addSlide({ masterName: "BLACK_LUXURY" });
if (fs.existsSync(assets.coverBlur)) {
  closing.addImage({
    path: assets.coverBlur,
    ...imageSizingCrop(assets.coverBlur, 0, 0, 13.333, 7.5),
  });
}
closing.addShape(pptx.ShapeType.rect, {
  x: 0,
  y: 0,
  w: 13.333,
  h: 7.5,
  line: { color: palette.stage, transparency: 100 },
  fill: { color: palette.stage, transparency: 52 },
});
addTopLabel(closing, "FINAL SLIDE");
closing.addText("THANK YOU", {
  x: 4.18,
  y: 2.1,
  w: 4.98,
  h: 0.42,
  placeholder: "officecli_title",
  fontFace: "Arial",
  fontSize: 30,
  color: palette.text,
  bold: true,
  align: "center",
  margin: 0,
});
closing.addText("黑底高级感来自清晰的层次、足够强的对比，以及被处理过的背景图，而不是简单把页面涂黑。", {
  x: 3.04,
  y: 2.82,
  w: 7.18,
  h: 0.36,
  fontFace: "Arial",
  fontSize: 11,
  color: palette.textSoft,
  align: "center",
  margin: 0,
});
addBottomGlow(closing);
finalizeSlide(closing, { skipOverlap: true });

async function writeDeck() {
  await pptx.writeFile({ fileName: "deck.pptx" });
  sanitizeGeneratedDeck("deck.pptx");
}

writeDeck().catch((error) => {
  console.error(error);
  process.exit(1);
});
