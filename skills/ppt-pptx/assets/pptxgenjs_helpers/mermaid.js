// Copyright (c) OpenAI. All rights reserved.
// Mermaid diagram helpers for PptxGenJS – render Mermaid text to SVG and embed in slides.
"use strict";

const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const os = require("os");
const { svgToDataUri } = require("./svg");
const { imageSizingContain } = require("./image");

/**
 * Render a Mermaid diagram string to an SVG string.
 *
 * Requires `mmdc` (mermaid-cli) to be installed:
 *   npm install -g @mermaid-js/mermaid-cli
 *
 * @param {string} mermaidText - Mermaid diagram source
 * @param {Object} [options]
 * @param {string} [options.theme] - Mermaid theme: "dark", "default", "forest", "neutral"
 * @param {string} [options.backgroundColor] - Background color (default: "transparent")
 * @param {number} [options.width] - Diagram width in pixels
 * @returns {string} SVG string
 */
function renderMermaidToSVG(mermaidText, options = {}) {
  const {
    theme = "dark",
    backgroundColor = "transparent",
    width = 1200,
  } = options;

  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "mermaid-"));
  const inputFile = path.join(tmpDir, "input.mmd");
  const outputFile = path.join(tmpDir, "output.svg");
  const configFile = path.join(tmpDir, "config.json");

  try {
    fs.writeFileSync(inputFile, mermaidText, "utf8");
    fs.writeFileSync(
      configFile,
      JSON.stringify({
        theme,
        themeVariables: {
          darkMode: theme === "dark",
          background: backgroundColor,
        },
      }),
      "utf8"
    );

    const cmd = [
      "mmdc",
      "-i", inputFile,
      "-o", outputFile,
      "-c", configFile,
      "-w", String(width),
      "--backgroundColor", backgroundColor,
    ].join(" ");

    execSync(cmd, { stdio: "pipe", timeout: 30000 });

    if (!fs.existsSync(outputFile)) {
      throw new Error("Mermaid CLI did not produce output SVG");
    }

    return fs.readFileSync(outputFile, "utf8");
  } finally {
    // Cleanup temp files
    try {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    } catch (_) {
      // Ignore cleanup errors
    }
  }
}

/**
 * Render a Mermaid diagram and save it as a local SVG file.
 *
 * @param {string} mermaidText - Mermaid diagram source
 * @param {string} outputPath - Where to save the SVG file
 * @param {Object} [options] - Same as renderMermaidToSVG options
 * @returns {string} The output path
 */
function renderMermaidToFile(mermaidText, outputPath, options = {}) {
  const svg = renderMermaidToSVG(mermaidText, options);
  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.writeFileSync(outputPath, svg, "utf8");
  return outputPath;
}

/**
 * Add a Mermaid diagram to a slide as an embedded SVG image.
 *
 * @param {Object} slide - PptxGenJS slide object
 * @param {string} mermaidText - Mermaid diagram source
 * @param {number} x - X position in inches
 * @param {number} y - Y position in inches
 * @param {number} w - Width in inches
 * @param {number} h - Height in inches
 * @param {Object} [options]
 * @param {string} [options.theme] - Mermaid theme
 * @param {string} [options.backgroundColor] - Background color
 * @param {string} [options.savePath] - If provided, also save SVG to this path
 */
function addMermaidDiagram(slide, mermaidText, x, y, w, h, options = {}) {
  const svgString = renderMermaidToSVG(mermaidText, {
    theme: options.theme,
    backgroundColor: options.backgroundColor,
    width: options.width,
  });

  if (options.savePath) {
    fs.mkdirSync(path.dirname(options.savePath), { recursive: true });
    fs.writeFileSync(options.savePath, svgString, "utf8");
  }

  const dataUri = svgToDataUri(svgString);

  slide.addImage({
    data: dataUri,
    ...imageSizingContain(dataUri, x, y, w, h),
  });
}

/**
 * Check if mermaid-cli (mmdc) is available.
 *
 * @returns {boolean}
 */
function isMermaidCLIAvailable() {
  try {
    execSync("mmdc --version", { stdio: "pipe", timeout: 5000 });
    return true;
  } catch (_) {
    return false;
  }
}

module.exports = {
  renderMermaidToSVG,
  renderMermaidToFile,
  addMermaidDiagram,
  isMermaidCLIAvailable,
};
