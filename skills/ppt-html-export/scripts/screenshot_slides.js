#!/usr/bin/env node
/**
 * Capture per-slide screenshots from an HTML presentation for QA.
 *
 * Usage:
 *   node screenshot_slides.js <input.html> [output_dir]
 *
 * Outputs one PNG per .slide element, named slide_01.png, slide_02.png, etc.
 * Useful for visual-review QA and montage generation.
 */

const puppeteer = require('puppeteer');
const path = require('path');
const fs = require('fs');

async function screenshotSlides(inputPath, outputDir) {
  if (!fs.existsSync(inputPath)) {
    console.error(`File not found: ${inputPath}`);
    process.exit(1);
  }

  const absoluteInput = path.resolve(inputPath);
  const dir = outputDir
    ? path.resolve(outputDir)
    : path.join(path.dirname(absoluteInput), 'rendered');

  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }

  const browser = await puppeteer.launch({ headless: 'new' });
  const page = await browser.newPage();
  await page.setViewport({ width: 1920, height: 1080 });

  await page.goto(`file://${absoluteInput}`, {
    waitUntil: 'networkidle0',
    timeout: 30000,
  });

  // Disable animations
  await page.addStyleTag({
    content: `
      *, *::before, *::after {
        animation-duration: 0s !important;
        transition-duration: 0s !important;
      }
    `,
  });

  const slideCount = await page.evaluate(() => {
    return document.querySelectorAll('.slide').length;
  });

  console.log(`Found ${slideCount} slides, capturing screenshots...`);

  for (let i = 0; i < slideCount; i++) {
    const slideSelector = `.slide:nth-child(${i + 1})`;
    const element = await page.$(slideSelector);

    if (element) {
      const filename = `slide_${String(i + 1).padStart(2, '0')}.png`;
      const outputPath = path.join(dir, filename);

      await element.screenshot({ path: outputPath });
      console.log(`  ✅ ${filename}`);
    } else {
      console.warn(`  ⚠️ Slide ${i + 1} not found with selector: ${slideSelector}`);
    }
  }

  await browser.close();
  console.log(`\n📸 ${slideCount} screenshots saved to: ${dir}`);
  return { slideCount, outputDir: dir };
}

// CLI entry point
const args = process.argv.slice(2);
if (args.length < 1) {
  console.log('Usage: node screenshot_slides.js <input.html> [output_dir]');
  process.exit(1);
}

screenshotSlides(args[0], args[1]).catch((err) => {
  console.error('Screenshot error:', err.message);
  process.exit(1);
});
