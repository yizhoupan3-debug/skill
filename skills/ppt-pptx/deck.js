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
  addStyledChart,
  addGlassPanel,
  getTypography,
  getSmartTypography,
} = require("./pptxgenjs_helpers");

const pptx = new pptxgen();
pptx.layout = "LAYOUT_WIDE";
pptx.title = "Auto-Pagination 压力测试";
pptx.lang = "zh-CN";

const palette = {
  "stage": "000000",
  "panel": "111111",
  "panelSoft": "171717",
  "line": "2A2A2A",
  "glow": "7EA9FF",
  "text": "F2F2EE",
  "textSoft": "B9B9B2",
  "textMute": "888883",
  "chip": "F4F4EF",
  "chipText": "111111"
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
  slide.addText(cn, { x, y, w: cnW, h: 0.24, placeholder: "officecli_title", ...getSmartTypography("h2", cn, cnW, 0.24, { color: palette.text, animate: { type: "fade", prop: "in" } }) });
  if (en) slide.addText(en, { x, y: y + 0.34, w, h: 0.14, ...getSmartTypography("body2", en, w, 0.14, { color: palette.textSoft, bold: true, animate: { type: "fade", prop: "in", delay: 0.1 } }) });
}

function finalizeSlide(slide, opts = {}) {
  if (!opts.skipOverlap) warnIfSlideHasOverlaps(slide, pptx, { ignoreDecorativeShapes: true });
  warnIfSlideElementsOutOfBounds(slide, pptx);
}

function addOptionalImage(slide, imagePath, sizingFactory, fallback = {}) {
  if (imagePath && fs.existsSync(imagePath)) {
    slide.addImage({ path: imagePath, ...sizingFactory(imagePath) });
    return true;
  }

  slide.addShape(pptx.ShapeType.rect, {
    x: fallback.x ?? 0,
    y: fallback.y ?? 0,
    w: fallback.w ?? 1,
    h: fallback.h ?? 1,
    line: { color: palette.line, transparency: 0, pt: 0.5 },
    fill: { color: palette.panelSoft, transparency: 0 },
  });
  slide.addText(fallback.label || "OPTIONAL IMAGE", {
    x: (fallback.x ?? 0) + 0.16,
    y: (fallback.y ?? 0) + 0.16,
    w: Math.max((fallback.w ?? 1) - 0.32, 0.8),
    h: 0.12,
    ...getTypography("caption", { color: palette.textMute }),
  });
  return false;
}

function resolveRustToolCommand(subcommand, args = []) {
  if (process.env.PPT_PPTX_RUST_TOOL_BIN) {
    return [process.env.PPT_PPTX_RUST_TOOL_BIN, subcommand, ...args];
  }

  const repoRoot = path.resolve(__dirname, "..", "..", "..");
  for (const candidate of [
    path.join(repoRoot, "rust_tools", "target", "release", "pptx_tool_rs"),
    path.join(repoRoot, "rust_tools", "target", "debug", "pptx_tool_rs"),
  ]) {
    if (fs.existsSync(candidate)) {
      return [candidate, subcommand, ...args];
    }
  }

  if (process.env.PPT_PPTX_RUST_TOOL_MANIFEST) {
    return ["cargo", "run", "--manifest-path", process.env.PPT_PPTX_RUST_TOOL_MANIFEST, "--", subcommand, ...args];
  }

  const manifest = path.join(repoRoot, "rust_tools", "pptx_tool_rs", "Cargo.toml");
  if (fs.existsSync(manifest)) {
    return ["cargo", "run", "--manifest-path", manifest, "--", subcommand, ...args];
  }

  throw new Error("Could not locate pptx_tool_rs binary or manifest");
}

function sanitizeGeneratedDeck(fileName) {
  const [command, ...args] = resolveRustToolCommand("sanitize-pptx", [fileName]);
  const completed = spawnSync(command, args, { stdio: "inherit" });
  if (completed.status !== 0) {
    throw new Error(`sanitize-pptx failed for ${fileName}`);
  }
}

const totalSlides = 6;

// ── Cover ──
const cover = pptx.addSlide({ masterName: "OFFICECLI_SEMANTIC" });
cover.background = { color: palette.stage };
addOptionalImage(cover, "./assets/cover.jpg", (assetPath) => imageSizingCrop(assetPath, 0, 0, 13.333, 7.5), {
  x: 0, y: 0, w: 13.333, h: 7.5, label: "COVER IMAGE OPTIONAL"
});
cover.addShape(pptx.ShapeType.rect, { x: 0, y: 0, w: 13.333, h: 7.5, line: { color: palette.stage, transparency: 100 }, fill: { color: palette.stage, transparency: 40 } });
cover.addShape(pptx.ShapeType.rect, { x: 0, y: 0, w: 6.1, h: 7.5, line: { color: palette.stage, transparency: 100 }, fill: { color: palette.stage, transparency: 22 } });
addTopLabel(cover, "PRESENTATION");
cover.addText("Auto-Pagination 压力测试", { x: 0.92, y: 1.76, w: 4.64, h: 1.06, placeholder: "officecli_title", ...getTypography("display", { color: palette.text, animate: { type: "fade", prop: "in", delay: 0.1 } }) });
cover.addText("验证超量卡片与超密集文本的自动拆分与缩小", { x: 0.96, y: 3.02, w: 4.48, h: 0.66, ...getTypography("body1", { color: palette.textSoft, animate: { type: "fade", prop: "in", delay: 0.3 } }) });
cover.addText("PPTX Engine / 2026-03-18", { x: 0.96, y: 4.48, w: 3.0, h: 0.14, ...getTypography("body2", { color: palette.textSoft, animate: { type: "fade", prop: "in", delay: 0.5 } }) });
cover.addText("01 / " + String(totalSlides).padStart(2, "0"), { x: 12.2, y: 7.03, w: 0.4, h: 0.12, ...getTypography("caption", { color: palette.textMute, align: "right" }) });
addBottomGlow(cover);
finalizeSlide(cover, { skipOverlap: true });

// ── Slide 2: 1. 超载多卡片测试 (1/2) (multi-card) ──
const slide0 = pptx.addSlide({ masterName: "OFFICECLI_SEMANTIC" });
slide0.background = { color: palette.stage };
addTopLabel(slide0, "SECTION 01");
addSectionTitle(slide0, "1. 超载多卡片测试 (1/2)", "", 0.92, 0.96, 5.0);
addGlassPanel(slide0, pptx, 0.94, 2.0, 2.62, 3.8, { fill: palette.panelSoft, transparency: 10 });
slide0.addText("01", { x: 1.12, y: 2.28, w: 0.4, h: 0.2, ...getTypography("h3", { color: palette.text, animate: { type: "fade", prop: "in", delay: 0.2 } }) });
slide0.addText("卡片一：我们的战略是在接下来的 Q3 和 Q4 获取 30% 增长。", { x: 1.12, y: 2.72, w: 2.26, h: 2.8, ...getSmartTypography("body2", "卡片一：我们的战略是在接下来的 Q3 和 Q4 获取 30% 增长。", 2.26, 2.8, { color: palette.textSoft, valign: "top", breakLine: true, animate: { type: "fade", prop: "in", delay: 0.30000000000000004 } }) });
addGlassPanel(slide0, pptx, 3.78, 2.0, 2.62, 3.8, { fill: palette.panelSoft, transparency: 10 });
slide0.addText("02", { x: 3.96, y: 2.28, w: 0.4, h: 0.2, ...getTypography("h3", { color: palette.text, animate: { type: "fade", prop: "in", delay: 0.4 } }) });
slide0.addText("卡片二：执行策略A需要在三线城市铺开完整的地推网络以获取先发优势。", { x: 3.96, y: 2.72, w: 2.26, h: 2.8, ...getSmartTypography("body2", "卡片二：执行策略A需要在三线城市铺开完整的地推网络以获取先发优势。", 2.26, 2.8, { color: palette.textSoft, valign: "top", breakLine: true, animate: { type: "fade", prop: "in", delay: 0.5 } }) });
addGlassPanel(slide0, pptx, 6.62, 2.0, 2.62, 3.8, { fill: palette.panelSoft, transparency: 10 });
slide0.addText("03", { x: 6.80, y: 2.28, w: 0.4, h: 0.2, ...getTypography("h3", { color: palette.text, animate: { type: "fade", prop: "in", delay: 0.6000000000000001 } }) });
slide0.addText("卡片三：目前竞品的市场份额是下降的，但私域很强。", { x: 6.80, y: 2.72, w: 2.26, h: 2.8, ...getSmartTypography("body2", "卡片三：目前竞品的市场份额是下降的，但私域很强。", 2.26, 2.8, { color: palette.textSoft, valign: "top", breakLine: true, animate: { type: "fade", prop: "in", delay: 0.7000000000000001 } }) });
addGlassPanel(slide0, pptx, 9.46, 2.0, 2.62, 3.8, { fill: palette.panelSoft, transparency: 10 });
slide0.addText("04", { x: 9.64, y: 2.28, w: 0.4, h: 0.2, ...getTypography("h3", { color: palette.text, animate: { type: "fade", prop: "in", delay: 0.8 } }) });
slide0.addText("卡片四：价格战不可避免，必须要用第二梯队产品来阻击", { x: 9.64, y: 2.72, w: 2.26, h: 2.8, ...getSmartTypography("body2", "卡片四：价格战不可避免，必须要用第二梯队产品来阻击", 2.26, 2.8, { color: palette.textSoft, valign: "top", breakLine: true, animate: { type: "fade", prop: "in", delay: 0.9 } }) });

slide0.addText("02 / " + String(totalSlides).padStart(2, "0"), { x: 12.2, y: 7.03, w: 0.4, h: 0.12, ...getTypography("caption", { color: palette.textMute, align: "right" }) });
addBottomGlow(slide0);
finalizeSlide(slide0);
slide0.addNotes("【第 2/10 页】1. 超载多卡片测试 (1/2)\n\n本页从多个维度展开分析，核心观点如下：\n  1. 卡片一：我们的战略是在接下来的 Q3 和 Q4 获取 30% 增长。\n  2. 卡片二：执行策略A需要在三线城市铺开完整的地推网络以获取先发优势。\n  3. 卡片三：目前竞品的市场份额是下降的，但私域很强。\n  4. 卡片四：价格战不可避免，必须要用第二梯队产品来阻击。\n\n请重点关注\"卡片一：我们的战略是在接下来的...\"，这是本节的核心立论。\n【过渡】讲完本页后，顺势引出下一部分内容，保持叙事连贯性。");

// ── Slide 3: 1. 超载多卡片测试 (2/2) (full-text) ──
const slide1 = pptx.addSlide({ masterName: "OFFICECLI_SEMANTIC" });
slide1.background = { color: palette.stage };
addTopLabel(slide1, "SECTION 02");
addSectionTitle(slide1, "1. 超载多卡片测试 (2/2)", "", 0.92, 0.96, 5.0);
addGlassPanel(slide1, pptx, 0.94, 1.76, 11.42, 4.44, { fill: palette.panelSoft, transparency: 10 });
slide1.addText("1. 卡片五（溢出）：这个重点应该被切割到第二页去，不要强行挤压前面的内容。\\n2. 卡片六（溢出）：团队的扩张速度赶不上业务，需要紧急引入顾问团队。", { x: 1.18, y: 2.04, w: 10.9, h: 3.88, ...getSmartTypography("body1", "1. 卡片五（溢出）：这个重点应该被切割到第二页去，不要强行挤压前面的内容。\\n2. 卡片六（溢出）：团队的扩张速度赶不上业务，需要紧急引入顾问团队。", 10.9, 3.88, { color: palette.textSoft, valign: "top", breakLine: true, animate: { type: "fade", prop: "in", delay: 0.2 } }) });

slide1.addText("03 / " + String(totalSlides).padStart(2, "0"), { x: 12.2, y: 7.03, w: 0.4, h: 0.12, ...getTypography("caption", { color: palette.textMute, align: "right" }) });
addBottomGlow(slide1);
finalizeSlide(slide1);
slide1.addNotes("【第 3/10 页】1. 超载多卡片测试 (2/2)\n\n本页核心内容：\n  1. 卡片五（溢出）：这个重点应该被切割到第二页去，不要强行挤压前面的内容。\n  2. 卡片六（溢出）：团队的扩张速度赶不上业务，需要紧急引入顾问团队。\n\n【过渡】讲完本页后，顺势引出下一部分内容，保持叙事连贯性。");

// ── Slide 4: 2. 超长文本拆分测试 (1/2) (hero-image) ──
const slide2 = pptx.addSlide({ masterName: "OFFICECLI_SEMANTIC" });
slide2.background = { color: palette.stage };
addTopLabel(slide2, "SECTION 03");
addSectionTitle(slide2, "2. 超长文本拆分测试 (1/2)", "", 0.92, 0.96, 5.0);
const slide2HasImage = addOptionalImage(slide2, "./assets/placeholder.jpg", (assetPath) => imageSizingCrop(assetPath, 0, 1.4, 13.333, 6.1), {
  x: 0, y: 1.4, w: 13.333, h: 6.1, label: "OPTIONAL IMAGE"
});
if (slide2HasImage) slide2.addShape(pptx.ShapeType.rect, { x: 0, y: 1.4, w: 13.333, h: 6.1, line: { color: palette.stage, transparency: 100 }, fill: { color: palette.stage, transparency: 40 } });


slide2.addText("04 / " + String(totalSlides).padStart(2, "0"), { x: 12.2, y: 7.03, w: 0.4, h: 0.12, ...getTypography("caption", { color: palette.textMute, align: "right" }) });
addBottomGlow(slide2);
finalizeSlide(slide2);
slide2.addNotes("【第 4/10 页】2. 超长文本拆分测试 (1/2)\n\n本页配合图像展开叙述，文字侧要点如下：\n  1. 第一段非常长：这种半图半文的结构如果被大量文字塞满会显得极其难看，甚至无法辨认。系统内置的重排逻辑会统计总字符数。当超过容忍阈值（例如单页250个中文字符时），这些段落将被完美切断并克隆当前页的图片产生连续汇报页。\n  2. 第二段还在写：为了证明阈值检测的有效性，我们需要尽量多写一些废话来触发条件。在传统的排版引擎里，这些字会被等比缩放到 5pt 导致放映机根本无法播放。\n\n建议在讲解文字时，引导听众视线从图像移向文字侧，形成注意力锚点转换。\n【过渡】讲完本页后，顺势引出下一部分内容，保持叙事连贯性。");

// ── Slide 5: 2. 超长文本拆分测试 (2/2) (hero-image) ──
const slide3 = pptx.addSlide({ masterName: "OFFICECLI_SEMANTIC" });
slide3.background = { color: palette.stage };
addTopLabel(slide3, "SECTION 04");
addSectionTitle(slide3, "2. 超长文本拆分测试 (2/2)", "", 0.92, 0.96, 5.0);
const slide3HasImage = addOptionalImage(slide3, "./assets/placeholder.jpg", (assetPath) => imageSizingCrop(assetPath, 0, 1.4, 13.333, 6.1), {
  x: 0, y: 1.4, w: 13.333, h: 6.1, label: "OPTIONAL IMAGE"
});
if (slide3HasImage) slide3.addShape(pptx.ShapeType.rect, { x: 0, y: 1.4, w: 13.333, h: 6.1, line: { color: palette.stage, transparency: 100 }, fill: { color: palette.stage, transparency: 40 } });


slide3.addText("05 / " + String(totalSlides).padStart(2, "0"), { x: 12.2, y: 7.03, w: 0.4, h: 0.12, ...getTypography("caption", { color: palette.textMute, align: "right" }) });
addBottomGlow(slide3);
finalizeSlide(slide3);
slide3.addNotes("【第 5/10 页】2. 超长文本拆分测试 (2/2)\n\n本页配合图像展开叙述，文字侧要点如下：\n  1. 第三段触发边界：如果我们再加上这第三百个字，那么这页幻灯片的命运就注定要被无情的一刀切断了，这也是为最终演示效果负责。引擎将接管一切。\n\n建议在讲解文字时，引导听众视线从图像移向文字侧，形成注意力锚点转换。\n【过渡】讲完本页后，顺势引出下一部分内容，保持叙事连贯性。");

// ── Slide 6: 3. 数据图表测试但附带超长标签 (data-panel) ──
const slide4 = pptx.addSlide({ masterName: "OFFICECLI_SEMANTIC" });
slide4.background = { color: palette.stage };
addTopLabel(slide4, "SECTION 05");
addSectionTitle(slide4, "3. 数据图表测试但附带超长标签", "", 0.92, 0.96, 5.0);
addMetricChip(slide4, 0.94, 2.3, 2.20, "30%", "这个极其夸张非常冗长根本不应该写在副标题里面的注记会被 SmartTypography 缩小。", 0.2);
addMetricChip(slide4, 3.36, 2.3, 2.20, "9%", "一般", 0.35);
addMetricChip(slide4, 5.78, 2.3, 2.20, "4.5", "核心", 0.5);
addGlassPanel(slide4, pptx, 0.94, 3.56, 11.42, 2.24, { fill: palette.panelSoft, transparency: 8 });
addStyledChart(slide4, pptx, "bar", {
  series: [{"name":"Revenue","values":[120,240,310]}],
  categories: ["2023","2024","2025"],
  position: { x: 1.1, y: 3.7, w: 11.1, h: 1.96 },
});

slide4.addText("06 / " + String(totalSlides).padStart(2, "0"), { x: 12.2, y: 7.03, w: 0.4, h: 0.12, ...getTypography("caption", { color: palette.textMute, align: "right" }) });
addBottomGlow(slide4);
finalizeSlide(slide4);
slide4.addNotes("【第 6/10 页】3. 数据图表测试但附带超长标签\n\n本页展示关键数据指标：\n  · 这个极其夸张非常冗长根本不应该写在副标题里面的注记会被 SmartTypography 缩小。：30%\n  · 一般：9%\n  · 核心：4.5\n\n数据来源请参见来源注释，演讲时重点强调最大差距或最高增幅的那条数字。\n图表类型：bar，包含 1 组数据系列。\n【过渡】讲完本页后，顺势引出下一部分内容，保持叙事连贯性。");

// ── Slide 7: 4. 智能字号衰减测试 (Smart Typography) (full-text) ──
const slide5 = pptx.addSlide({ masterName: "OFFICECLI_SEMANTIC" });
slide5.background = { color: palette.stage };
addTopLabel(slide5, "SECTION 06");
addSectionTitle(slide5, "4. 智能字号衰减测试 (Smart Typography)", "", 0.92, 0.96, 5.0);
addGlassPanel(slide5, pptx, 0.94, 1.76, 11.42, 4.44, { fill: palette.panelSoft, transparency: 10 });
slide5.addText("1. 这里有一段文字非常密集：通常，我们期望每张卡片不超过 40 个字。但如果用户非要输入超过 80 个字，比如我们现在做的这样。由于卡片不足 5 张，这个幻灯片（也就是这页）不会被触发 Pagination（自动分页），但是这么大的信息量将会溢出这单张卡片的矩形高度。这时候 SmartTypography 工具类会出手，估算这段文字加上间距后需要的高度，并在发现其超过容器 90% 水位线时，激进地自动将其降级，同时打开 pptxgenjs 的 autoFit=true 兜底。一切都为了这行字不越\\n2. ", { x: 1.18, y: 2.04, w: 10.9, h: 3.88, ...getSmartTypography("body1", "1. 这里有一段文字非常密集：通常，我们期望每张卡片不超过 40 个字。但如果用户非要输入超过 80 个字，比如我们现在做的这样。由于卡片不足 5 张，这个幻灯片（也就是这页）不会被触发 Pagination（自动分页），但是这么大的信息量将会溢出这单张卡片的矩形高度。这时候 SmartTypography 工具类会出手，估算这段文字加上间距后需要的高度，并在发现其超过容器 90% 水位线时，激进地自动将其降级，同时打开 pptxgenjs 的 autoFit=true 兜底。一切都为了这行字不越\\n2. ", 10.9, 3.88, { color: palette.textSoft, valign: "top", breakLine: true, animate: { type: "fade", prop: "in", delay: 0.2 } }) });

slide5.addText("07 / " + String(totalSlides).padStart(2, "0"), { x: 12.2, y: 7.03, w: 0.4, h: 0.12, ...getTypography("caption", { color: palette.textMute, align: "right" }) });
addBottomGlow(slide5);
finalizeSlide(slide5);
slide5.addNotes("【第 7/10 页】4. 智能字号衰减测试 (Smart Typography)\n\n本页核心内容：\n  1. 这里有一段文字非常密集：通常，我们期望每张卡片不超过 40 个字。但如果用户非要输入超过 80 个字，比如我们现在做的这样。由于卡片不足 5 张，这个幻灯片（也就是这页）不会被触发 Pagination（自动分页），但是这么大的信息量将会溢出这单张卡片的矩形高度。这时候 SmartTypography 工具类会出手，估算这段文字加上间距后需要的高度，并在发现其超过容器 90% 水位线时，激进地自动将其降级，同时打开 pptxgenjs 的 autoFit=true 兜底。一切都为了这行字不越界。\n  2. 正常文本\n\n【过渡】讲完本页后，顺势引出下一部分内容，保持叙事连贯性。");

// ── Closing ──
const closing = pptx.addSlide({ masterName: "OFFICECLI_SEMANTIC" });
closing.background = { color: palette.stage };
addOptionalImage(closing, "./assets/cover.jpg", (assetPath) => imageSizingCrop(assetPath, 0, 0, 13.333, 7.5), {
  x: 0, y: 0, w: 13.333, h: 7.5, label: "COVER IMAGE OPTIONAL"
});
closing.addShape(pptx.ShapeType.rect, { x: 0, y: 0, w: 13.333, h: 7.5, line: { color: palette.stage, transparency: 100 }, fill: { color: palette.stage, transparency: 52 } });
addTopLabel(closing, "FINAL SLIDE");
closing.addText("THANK YOU", { x: 4.18, y: 2.1, w: 4.98, h: 0.42, placeholder: "officecli_title", ...getTypography("display", { color: palette.text, align: "center", animate: { type: "fade", prop: "in", delay: 0.2 } }) });
closing.addText("" + String(totalSlides).padStart(2, "0") + " / " + String(totalSlides).padStart(2, "0"), { x: 12.2, y: 7.03, w: 0.4, h: 0.12, ...getTypography("caption", { color: palette.textMute, align: "right" }) });
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
