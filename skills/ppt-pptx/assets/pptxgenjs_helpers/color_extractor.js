// Copyright (c) OpenAI. All rights reserved.
"use strict";

const fs = require("fs");

let Canvas, loadImage;
try {
  const skia = require("skia-canvas");
  Canvas = skia.Canvas;
  loadImage = skia.loadImage;
} catch (e) {
  // If skia-canvas fails, fallback methods can be provided or it fails gracefully
}

/**
 * Converts r, g, b to a Hex string (no '#')
 */
function rgbToHex(r, g, b) {
  return [r, g, b].map(x => {
    const hex = x.toString(16);
    return hex.length === 1 ? '0' + hex : hex;
  }).join('').toUpperCase();
}

/**
 * Extracts the dominant/average color of an image using skia-canvas.
 * Downsamples to 10x10 and averages the RGB values.
 * Returns a Hex string (e.g. "FF55A3") or null if failed.
 * 
 * @param {string} imagePath - Path to the local image
 * @returns {Promise<string|null>}
 */
async function getDominantColor(imagePath) {
  if (!Canvas || !loadImage) {
    console.warn("skia-canvas is not available for color extraction.");
    return null;
  }

  try {
    if (!fs.existsSync(imagePath)) {
      return null;
    }
    const img = await loadImage(imagePath);
    const canvas = new Canvas(10, 10);
    const ctx = canvas.getContext("2d");
    
    // Draw the image scaled down to 10x10
    ctx.drawImage(img, 0, 0, 10, 10);
    const imgData = ctx.getImageData(0, 0, 10, 10).data;
    
    let r = 0, g = 0, b = 0, count = 0;
    
    // Each pixel is 4 bytes: R, G, B, A
    for (let i = 0; i < imgData.length; i += 4) {
      const alpha = imgData[i + 3];
      if (alpha > 128) { // ignore overly transparent pixels
        r += imgData[i];
        g += imgData[i + 1];
        b += imgData[i + 2];
        count++;
      }
    }
    
    if (count === 0) return null;

    r = Math.round(r / count);
    g = Math.round(g / count);
    b = Math.round(b / count);

    // Boost saturation + brightness slightly to make it suitable as a glow/accent color
    // A simple linear boost towards white for dark colors, or just flat +20 value
    const hex = rgbToHex(
      Math.min(255, r + 30),
      Math.min(255, g + 30),
      Math.min(255, b + 30)
    );
    return hex;
  } catch (err) {
    console.warn(`Could not extract color from ${imagePath}: ${err.message}`);
    return null;
  }
}

module.exports = {
  getDominantColor,
};
