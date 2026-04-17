// Copyright (c) OpenAI. All rights reserved.
"use strict";

const VERSION = "1.4.0";

const text = require("./text");
const image = require("./image");
const svg = require("./svg");
const latex = require("./latex");
const code = require("./code");
const layout = require("./layout");
const layoutBuilders = require("./layout_builders");
const util = require("./util");
const chartBuilders = require("./chart_builders");
const mermaid = require("./mermaid");
const typography = require("./typography");
const glassmorphism = require("./glassmorphism");
const colorExtractor = require("./color_extractor");
const textReflow = require("./text_reflow");
const speakerNotes = require("./speaker_notes");

module.exports = {
  VERSION,
  // text layout
  ...text,
  // images
  ...image,
  // svg helpers
  ...svg,
  // LaTeX -> SVG
  ...latex,
  // code block -> pptx text runs
  ...code,
  // slide layout analyzers
  ...layout,
  // slide layout builders
  ...layoutBuilders,
  // text layout helpers and utilities
  ...util,
  // data-driven chart builders
  ...chartBuilders,
  // mermaid diagram helpers
  ...mermaid,
  // typography
  ...typography,
  getSmartTypography: typography.getSmartTypography,
  // glassmorphism
  ...glassmorphism,
  // ambient color
  ...colorExtractor,
  // text reflow / orphan fixer
  ...textReflow,
  // speaker notes generator
  ...speakerNotes,
};
