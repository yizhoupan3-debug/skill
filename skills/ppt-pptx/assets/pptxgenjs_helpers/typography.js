// Copyright (c) OpenAI. All rights reserved.
"use strict";

// Use a conservative cross-platform sans serif that exists on both macOS and Windows.
// For CJK text, PowerPoint/LibreOffice may still fall back per platform, but we avoid
// hardcoding platform-specific authoring fonts such as Helvetica Neue or Calibri.
const CROSS_PLATFORM_SANS = "Arial";

const TYPOGRAPHIC_SCALE = {
  display:  { fontFace: CROSS_PLATFORM_SANS, fontSize: 36,   bold: true,  charSpace: 1.5, margin: 0 },
  h1:       { fontFace: CROSS_PLATFORM_SANS, fontSize: 26,   bold: true,  charSpace: 1.0, margin: 0 },
  h2:       { fontFace: CROSS_PLATFORM_SANS, fontSize: 20,   bold: true,  charSpace: 0.8, margin: 0 },
  h3:       { fontFace: CROSS_PLATFORM_SANS, fontSize: 16,   bold: true,  charSpace: 0.5, margin: 0 },
  body1:    { fontFace: CROSS_PLATFORM_SANS, fontSize: 11.5, bold: false, lineSpacing: 16, margin: 0 },
  body2:    { fontFace: CROSS_PLATFORM_SANS, fontSize: 10.5, bold: false, lineSpacing: 15, margin: 0 },
  caption:  { fontFace: CROSS_PLATFORM_SANS, fontSize: 9.0,  bold: false, charSpace: 0.5, margin: 0 },
  overline: { fontFace: CROSS_PLATFORM_SANS, fontSize: 8.5,  bold: true,  charSpace: 1.5, margin: 0 },
  metric:   { fontFace: CROSS_PLATFORM_SANS, fontSize: 18,   bold: true,  charSpace: 0.0, margin: 0 },
};

/**
 * Returns a typography configuration object for PptxGenJS.
 * 
 * @param {string} scale - The typographic scale name ('h1', 'body1', etc.)
 * @param {Object} [overrides] - Specific options to override
 * @returns {Object} PptxGenJS text options
 */
function getTypography(scale, overrides = {}) {
  const base = TYPOGRAPHIC_SCALE[scale] || TYPOGRAPHIC_SCALE.body1;
  return { ...base, ...overrides };
}

/**
 * Estimates physical text bounds and dynamically shrinks font size if it exceeds the container.
 * 
 * @param {string} scale - Typographic scale
 * @param {string} text - The actual text string
 * @param {number} containerWidth - Container width in inches
 * @param {number} containerHeight - Container height in inches
 * @param {Object} [overrides]
 * @returns {Object} Config with potentially modified fontSize and autoFit
 */
function getSmartTypography(scale, text, containerWidth, containerHeight, overrides = {}) {
  const base = getTypography(scale, overrides);
  if (!text || !containerWidth || !containerHeight) return base;

  // Very rough estimation: average character width is ~ 60% of font size (in points).
  // 1 inch = 72 points.
  const safeText = String(text);
  const fontSizePt = base.fontSize || 12;
  const avgCharWidthPt = fontSizePt * 0.6;
  const containerWidthPt = containerWidth * 72;
  
  // How many characters fit on one line?
  const charsPerLine = Math.max(1, Math.floor(containerWidthPt / avgCharWidthPt));
  
  // Estimate total lines needed
  const lines = safeText.split("\n");
  let estimatedLines = 0;
  for (const line of lines) {
    estimatedLines += Math.ceil((line.length || 1) / charsPerLine);
  }

  // Estimated total height in inches
  const lineHeightRatio = (base.lineSpacing || fontSizePt * 1.2) / fontSizePt;
  const estimatedHeightPt = estimatedLines * (fontSizePt * lineHeightRatio);
  const estimatedHeightInches = estimatedHeightPt / 72;

  // If the text height exceeds the container height, we scale it down.
  if (estimatedHeightInches > containerHeight * 0.9) {
    // Determine a safe scale factor (max shrink is ~ 0.75x)
    const scaleFactor = Math.max(0.75, (containerHeight * 0.9) / estimatedHeightInches);
    
    // Apply downscaling
    base.fontSize = Math.floor(fontSizePt * scaleFactor * 10) / 10;
    
    // Enable autoFit natively as a fallback safety net
    base.autoFit = true;
    
    if (scaleFactor <= 0.8) {
      console.warn(`[Typography] Auto-scaled overloaded text down closely to ${base.fontSize}pt`);
    }
  }

  return base;
}

module.exports = {
  getTypography,
  getSmartTypography,
  TYPOGRAPHIC_SCALE,
  CROSS_PLATFORM_SANS,
};
