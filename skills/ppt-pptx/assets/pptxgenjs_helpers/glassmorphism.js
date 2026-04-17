// Copyright (c) OpenAI. All rights reserved.
"use strict";

const { safeOuterShadow } = require("./util");

/**
 * Simulates a glassmorphism frosted panel with depth and a fine rim light.
 * 
 * @param {Object} slide - PptxGenJS slide
 * @param {Object} pptx - PptxGenJS instance
 * @param {number} x - x position
 * @param {number} y - y position
 * @param {number} w - width
 * @param {number} h - height
 * @param {Object} [options]
 */
function addGlassPanel(slide, pptx, x, y, w, h, options = {}) {
  const {
    radius = 0.08,
    fill = "111111",            // Dark gray base
    transparency = 20,          // % transparent (80% opacity)
    highlightColor = "F2F2EE",  // White/Light rim light
    highlightTransparency = 85, // Very subtle, 15% opacity rim
    shadowOpacity = 0.25,
  } = options;

  // 1. Main background (dark + transparent + shadow)
  slide.addShape(pptx.ShapeType.roundRect, {
    x, y, w, h,
    rectRadius: radius,
    line: { type: "none" },
    fill: { color: fill, transparency: transparency },
    shadow: safeOuterShadow("000000", shadowOpacity, 90, 2.0, 1.2),
  });

  // 2. Inner rim highlight (cut glass effect)
  slide.addShape(pptx.ShapeType.roundRect, {
    x: x + 0.01,
    y: y + 0.01,
    w: w - 0.02,
    h: h - 0.02,
    rectRadius: Math.max(radius - 0.01, 0),
    line: { color: highlightColor, pt: 0.5, transparency: highlightTransparency },
    fill: { type: "none" },
  });
}

module.exports = {
  addGlassPanel,
};
