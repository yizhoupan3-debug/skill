// Copyright (c) OpenAI. All rights reserved.
// Chart builder helpers for PptxGenJS – data-driven, palette-aware chart generation.
"use strict";

/**
 * Default chart palette – overridden by passing palette in options.
 * Colors are hex strings without '#'.
 */
const DEFAULT_CHART_PALETTE = [
  "7EA9FF", // blue glow
  "FF6B4A", // warm accent
  "41DB84", // green
  "F2C057", // gold
  "B77FFF", // purple
  "4FC1E9", // sky
  "FF8EB3", // pink
  "97D9A3", // mint
];

/**
 * Build a styled chart config object for pptx.addChart().
 *
 * @param {string} chartType - One of: "bar", "line", "pie", "doughnut", "area", "scatter"
 * @param {Object} config
 * @param {Array<{name: string, values: number[]}>} config.series - Chart data series
 * @param {string[]} [config.categories] - Category labels (x-axis)
 * @param {Object} [config.position] - {x, y, w, h} in inches
 * @param {string[]} [config.palette] - Hex color array (no '#')
 * @param {Object} [config.options] - Additional PptxGenJS chart options
 * @returns {Object} config suitable for slide.addChart(pptx.ChartType[type], data, opts)
 */
function buildChartConfig(chartType, config = {}) {
  const {
    series = [],
    categories = [],
    position = { x: 0.5, y: 1.5, w: 8, h: 4.5 },
    palette = DEFAULT_CHART_PALETTE,
    options = {},
  } = config;

  const chartTypeMap = {
    bar: "bar",
    bar3d: "bar3D",
    line: "line",
    pie: "pie",
    doughnut: "doughnut",
    area: "area",
    scatter: "scatter",
    radar: "radar",
  };

  const pptxType = chartTypeMap[chartType.toLowerCase()] || "bar";

  // Assign palette colors to series
  const coloredSeries = series.map((s, i) => ({
    name: s.name || `Series ${i + 1}`,
    labels: categories.length > 0 ? categories : undefined,
    values: s.values || [],
    color: palette[i % palette.length],
  }));

  // Build chart options with sensible defaults for a dark-luxury deck
  const chartOpts = {
    x: position.x,
    y: position.y,
    w: position.w,
    h: position.h,

    showLegend: series.length > 1,
    legendPos: "b",
    legendFontSize: 9,
    legendColor: "B9B9B2",

    showTitle: false,

    catAxisLabelColor: "B9B9B2",
    catAxisLabelFontSize: 9,
    catAxisLineShow: true,
    catAxisLineColor: "2A2A2A",

    valAxisLabelColor: "B9B9B2",
    valAxisLabelFontSize: 9,
    valAxisLineShow: false,
    valAxisMajorGridColor: "1A1A1A",

    plotArea: { fill: { color: "111111" }, border: { color: "2A2A2A", pt: 0.5 } },

    dataLabelColor: "F2F2EE",
    dataLabelFontSize: 8,

    ...options,
  };

  // Pie/doughnut-specific defaults
  if (pptxType === "pie" || pptxType === "doughnut") {
    chartOpts.showLegend = true;
    chartOpts.legendPos = "r";
    chartOpts.dataLabelPosition = "outEnd";
    chartOpts.showPercent = true;
    chartOpts.showValue = false;
  }

  return {
    type: pptxType,
    data: coloredSeries,
    options: chartOpts,
  };
}

/**
 * Add a styled chart to a slide, using the dark-luxury defaults.
 *
 * @param {Object} slide - PptxGenJS slide object
 * @param {Object} pptx - PptxGenJS instance (for ChartType enum)
 * @param {string} chartType - "bar", "line", "pie", etc.
 * @param {Object} config - Same as buildChartConfig config
 */
function addStyledChart(slide, pptx, chartType, config = {}) {
  const { type, data, options } = buildChartConfig(chartType, config);

  const chartTypeEnum = {
    bar: pptx.ChartType?.bar || "bar",
    bar3D: pptx.ChartType?.bar3D || "bar3D",
    line: pptx.ChartType?.line || "line",
    pie: pptx.ChartType?.pie || "pie",
    doughnut: pptx.ChartType?.doughnut || "doughnut",
    area: pptx.ChartType?.area || "area",
    scatter: pptx.ChartType?.scatter || "scatter",
    radar: pptx.ChartType?.radar || "radar",
  };

  slide.addChart(chartTypeEnum[type] || type, data, options);
}

/**
 * Build chart data from a simple CSV string.
 * First row = headers (category labels), subsequent rows = series.
 *
 * @param {string} csvString
 * @returns {{categories: string[], series: {name: string, values: number[]}[]}}
 */
function parseCSVToChartData(csvString) {
  const lines = csvString.trim().split("\n").map((l) => l.split(",").map((c) => c.trim()));
  if (lines.length < 2) return { categories: [], series: [] };

  const headers = lines[0];
  const categories = headers.slice(1);
  const series = [];

  for (let i = 1; i < lines.length; i++) {
    const row = lines[i];
    series.push({
      name: row[0] || `Series ${i}`,
      values: row.slice(1).map((v) => parseFloat(v) || 0),
    });
  }

  return { categories, series };
}

/**
 * Build chart data from a JSON array of objects.
 * Each object = one data point; keys become series names.
 *
 * @param {Object[]} jsonArray - e.g. [{category: "A", sales: 10, cost: 5}, ...]
 * @param {string} categoryKey - The key to use for category labels (default: first key)
 * @returns {{categories: string[], series: {name: string, values: number[]}[]}}
 */
function parseJSONToChartData(jsonArray, categoryKey = null) {
  if (!Array.isArray(jsonArray) || jsonArray.length === 0) {
    return { categories: [], series: [] };
  }

  const keys = Object.keys(jsonArray[0]);
  const catKey = categoryKey || keys[0];
  const valueKeys = keys.filter((k) => k !== catKey);

  const categories = jsonArray.map((obj) => String(obj[catKey] || ""));
  const series = valueKeys.map((key) => ({
    name: key,
    values: jsonArray.map((obj) => parseFloat(obj[key]) || 0),
  }));

  return { categories, series };
}

module.exports = {
  DEFAULT_CHART_PALETTE,
  buildChartConfig,
  addStyledChart,
  parseCSVToChartData,
  parseJSONToChartData,
};
