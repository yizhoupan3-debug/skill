/**
 * speaker_notes.js
 *
 * Generates concise, presentation-ready speaker notes from a slide object.
 * The notes are injected via slide.addNotes() in the generated deck.js.
 */
"use strict";

/**
 * Generates speaker notes text for a given slide object based on its pattern.
 *
 * @param {Object} slide - The slide data object from the YAML outline
 * @param {string} pattern - Detected pattern (multi-card, comparison, etc.)
 * @param {number} slideNum - Slide number for reference
 * @param {number} totalSlides - Total number of slides
 * @returns {string} Speaker notes text (plain text, multiline OK)
 */
function generateSpeakerNotes(slide, pattern, slideNum, totalSlides) {
  const title = slide.title || "本页";
  const subtitle = slide.subtitle ? `（${slide.subtitle}）` : "";
  const lines = [];

  // --- Opening transition ---
  lines.push(`【第 ${slideNum}/${totalSlides} 页】${title}${subtitle}`);
  lines.push("");

  switch (pattern) {
    case "multi-card": {
      const bullets = slide.bullets || [];
      if (bullets.length > 0) {
        lines.push("本页从多个维度展开分析，核心观点如下：");
        bullets.forEach((b, i) => {
          lines.push(`  ${i + 1}. ${b}`);
        });
        lines.push("");
        lines.push("请重点关注" + (bullets[0] ? `"${bullets[0].slice(0, 15)}..."` : "第一点") + "，这是本节的核心立论。");
      }
      break;
    }

    case "comparison": {
      const left = slide.comparison?.left || {};
      const right = slide.comparison?.right || {};
      lines.push(`本页进行左右对比分析：`);
      lines.push(`  ▶ 左侧【${left.title || "A"}】：${(left.items || []).join("；")}`);
      lines.push(`  ▶ 右侧【${right.title || "B"}】：${(right.items || []).join("；")}`);
      lines.push("");
      lines.push("两者的差异说明了不同策略路径下的权衡，建议结合实际选取最优方案。");
      break;
    }

    case "data-panel": {
      const metrics = slide.metrics || [];
      if (metrics.length > 0) {
        lines.push("本页展示关键数据指标：");
        metrics.forEach((m) => {
          lines.push(`  · ${m.label || "指标"}：${m.value || "-"}`);
        });
        lines.push("");
        lines.push("数据来源请参见来源注释，演讲时重点强调最大差距或最高增幅的那条数字。");
      }
      if (slide.chart) {
        lines.push(`图表类型：${slide.chart.type || "bar"}，包含 ${(slide.chart.series || []).length} 组数据系列。`);
      }
      break;
    }

    case "image-text-split":
    case "hero-image": {
      const bullets = slide.bullets || [];
      lines.push("本页配合图像展开叙述，文字侧要点如下：");
      bullets.forEach((b, i) => {
        lines.push(`  ${i + 1}. ${b}`);
      });
      lines.push("");
      lines.push("建议在讲解文字时，引导听众视线从图像移向文字侧，形成注意力锚点转换。");
      break;
    }

    case "timeline": {
      const tl = slide.timeline || [];
      lines.push("本页展示时间线：");
      tl.forEach((item) => {
        lines.push(`  ${item.year || item.date} → ${item.event || item.label || ""}`);
      });
      lines.push("");
      lines.push("时间线请按顺序逐点点击出现，可以让观众跟着节拍感受历史脉络。");
      break;
    }

    case "process-flow": {
      const steps = slide.steps || [];
      lines.push("本页展示流程步骤：");
      steps.forEach((step, i) => {
        lines.push(`  步骤 ${i + 1}：${typeof step === "string" ? step : step.label || ""}`);
      });
      lines.push("");
      lines.push("建议逐步骤动画点出，不要一次性全部展开，确保听众能跟上逻辑节拍。");
      break;
    }

    default: {
      // full-text
      const bullets = slide.bullets || [];
      if (bullets.length > 0) {
        lines.push("本页核心内容：");
        bullets.forEach((b, i) => lines.push(`  ${i + 1}. ${b}`));
        lines.push("");
      }
      break;
    }
  }

  // --- Closing transition to next slide ---
  if (slideNum < totalSlides - 1) {
    lines.push("【过渡】讲完本页后，顺势引出下一部分内容，保持叙事连贯性。");
  } else {
    lines.push("【收尾】本页是内容部分的最后一页，即将进入结语与致谢环节。");
  }

  return lines.join("\n");
}

module.exports = { generateSpeakerNotes };
