#!/usr/bin/env node
/**
 * Export HTML slides to PDF using Puppeteer.
 *
 * Usage:
 *   node export_pdf.js <input.html> [output.pdf]
 *
 * Waits for fonts and images to load, disables animations, then exports
 * a pixel-perfect PDF matching the 1920x1080 slide canvas.
 */

const puppeteer = require('puppeteer');
const path = require('path');
const fs = require('fs');

async function exportPdf(inputPath, outputPath) {
  if (!fs.existsSync(inputPath)) {
    console.error(`File not found: ${inputPath}`);
    process.exit(1);
  }

  const absoluteInput = path.resolve(inputPath);
  const absoluteOutput = outputPath
    ? path.resolve(outputPath)
    : absoluteInput.replace(/\.html?$/i, '.pdf');

  console.log(`Exporting: ${absoluteInput}`);
  console.log(`Output:    ${absoluteOutput}`);

  const browser = await puppeteer.launch({ headless: 'new' });
  const page = await browser.newPage();

  // Set viewport to match slide canvas
  await page.setViewport({ width: 1920, height: 1080 });

  // Navigate and wait for network idle (fonts, images)
  await page.goto(`file://${absoluteInput}`, {
    waitUntil: 'networkidle0',
    timeout: 30000,
  });

  // Disable CSS animations and transitions for clean export
  await page.addStyleTag({
    content: `
      *, *::before, *::after {
        animation-duration: 0s !important;
        animation-delay: 0s !important;
        transition-duration: 0s !important;
        transition-delay: 0s !important;
      }
    `,
  });

  // Wait a moment for style injection
  await new Promise((r) => setTimeout(r, 500));

  // Count slides for validation
  const slideCount = await page.evaluate(() => {
    return document.querySelectorAll('.slide').length;
  });
  console.log(`Found ${slideCount} slides`);

  // Export to PDF
  await page.pdf({
    path: absoluteOutput,
    width: '1920px',
    height: '1080px',
    printBackground: true,
    preferCSSPageSize: true,
    margin: { top: 0, right: 0, bottom: 0, left: 0 },
  });

  // Validate PDF was created
  if (fs.existsSync(absoluteOutput)) {
    const stats = fs.statSync(absoluteOutput);
    console.log(`✅ PDF exported: ${absoluteOutput} (${(stats.size / 1024).toFixed(1)} KB)`);
  } else {
    console.error('❌ PDF export failed');
    process.exit(1);
  }

  await browser.close();
  return { slideCount, outputPath: absoluteOutput };
}

// CLI entry point
const args = process.argv.slice(2);
if (args.length < 1) {
  console.log('Usage: node export_pdf.js <input.html> [output.pdf]');
  process.exit(1);
}

exportPdf(args[0], args[1]).catch((err) => {
  console.error('Export error:', err.message);
  process.exit(1);
});
