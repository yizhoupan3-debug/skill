/**
 * text_reflow.js
 *
 * Utility functions to prevent typography orphans and widows in
 * generated PowerPoint slide text. Orphan = a line with only 1-2 characters.
 */
"use strict";

/**
 * Estimates how many characters fit on one line given a container width and font size.
 * Uses ~0.55 * fontSize as avg char width for CJK/Latin mixed content.
 *
 * @param {number} containerWidthInch - Container width in inches
 * @param {number} fontSizePt - Font size in pt
 * @returns {number} Estimated characters per line
 */
function estimateCharsPerLine(containerWidthInch, fontSizePt = 11.5) {
  const pxPerChar = fontSizePt * 0.55;
  const containerWidthPt = containerWidthInch * 72;
  return Math.max(8, Math.floor(containerWidthPt / pxPerChar));
}

/**
 * Checks if the last logical line of the input text has too few characters (orphan/widow).
 *
 * @param {string} text - Input text (may contain \n)
 * @param {number} containerWidthInch - Container width in inches
 * @param {number} fontSizePt - Font size in pt
 * @param {number} minLineChars - Minimum acceptable trailing line chars (default 4)
 * @returns {boolean} Whether an orphan exists
 */
function hasOrphan(text, containerWidthInch, fontSizePt = 11.5, minLineChars = 4) {
  const charsPerLine = estimateCharsPerLine(containerWidthInch, fontSizePt);
  const paragraphs = text.split("\n");
  
  for (const para of paragraphs) {
    if (!para.trim()) continue;
    const remainder = para.length % charsPerLine;
    if (remainder > 0 && remainder <= minLineChars) {
      return true;
    }
  }
  return false;
}

/**
 * Attempts to fix orphaned short trailing lines by trimming or appending
 * a neutral CJK filler phrase at the end of each offending paragraph.
 *
 * Strategy:
 * - If orphan chars < threshold: try removing X trailing chars to push remainder up
 * - If that leaves a single-char tail: try adding a short neutral phrase instead
 *
 * @param {string} text - The text to process (may have \n separators for bullets)
 * @param {number} containerWidthInch - Container width in inches
 * @param {number} fontSizePt - Font size in pt  
 * @param {number} minLineChars - Minimum acceptable chars on last line (default 4)
 * @returns {string} Cleaned text with orphan lines resolved
 */
function fixOrphans(text, containerWidthInch, fontSizePt = 11.5, minLineChars = 4) {
  if (!text || !containerWidthInch) return text;

  const charsPerLine = estimateCharsPerLine(containerWidthInch, fontSizePt);
  const paragraphs = text.split("\n");

  const fixed = paragraphs.map((para) => {
    if (!para.trim()) return para;

    const len = para.length;
    const remainder = len % charsPerLine;

    // No orphan: the last line is full (remainder 0) or long enough
    if (remainder === 0 || remainder > minLineChars) return para;

    // Strategy A: Remove trailing chars to move content up to prior line
    // We need to remove (remainder) chars so the last portion wraps up
    const charsToRemove = remainder;
    const trimmed = para.slice(0, len - charsToRemove).trimEnd();
    
    // Verify trimmed version doesn't create a new orphan
    const newRemainder = trimmed.length % charsPerLine;
    if (newRemainder === 0 || newRemainder > minLineChars) {
      return trimmed;
    }

    // Strategy B: Append a short neutral phrase to shift the orphan forward
    const fillers = ["等方面均有影响", "需综合考量", "值得关注", "显著提升", "持续优化"];
    for (const filler of fillers) {
      const padded = para + filler;
      const paddedRemainder = padded.length % charsPerLine;
      if (paddedRemainder > minLineChars) {
        return padded;
      }
    }

    // Fallback: return original if no strategy worked
    return para;
  });

  return fixed.join("\n");
}

/**
 * Wraps fixOrphans over an array of bullet strings, processing each individually.
 *
 * @param {string[]} bullets - Array of bullet text strings
 * @param {number} containerWidthInch - Container width in inches
 * @param {number} fontSizePt - Font size in pt
 * @returns {string[]} Cleaned bullets array
 */
function fixBulletOrphans(bullets, containerWidthInch, fontSizePt = 11.5) {
  if (!Array.isArray(bullets)) return bullets;
  return bullets.map((b) => fixOrphans(b, containerWidthInch, fontSizePt));
}

module.exports = {
  fixOrphans,
  fixBulletOrphans,
  hasOrphan,
  estimateCharsPerLine,
};
