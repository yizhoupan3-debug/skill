#!/usr/bin/env node

const fs = require("fs");

const target = process.argv[2];

if (!target) {
  console.error("Usage: node scripts/patch_aionui_effort_ui.js <bundle-file>");
  process.exit(1);
}

const source = fs.readFileSync(target, "utf8");

const before =
  'i.createElement("label",{className:"flex items-center gap-4px rounded-full border px-8px py-2px text-12px",title:"推理强度"},i.createElement("span",{className:"whitespace-nowrap opacity-70"},"推理"),i.createElement("select",{value:effort,onChange:Ht,disabled:!d?.useModel,className:"bg-transparent outline-none text-12px",style:{border:"none",background:"transparent"}},effortLevels.map(e=>i.createElement("option",{key:e,value:e},e))))';

const after =
  'i.createElement("label",{className:"sendbox-model-btn header-model-btn agent-mode-compact-pill inline-flex items-center gap-6px h-28px px-10px text-12px leading-none",title:"推理强度"},i.createElement("span",{className:"whitespace-nowrap opacity-70"},"推理"),i.createElement("select",{value:effort,onChange:Ht,disabled:!d?.useModel,className:"min-w-0 bg-transparent outline-none text-12px leading-none",style:{border:"none",background:"transparent"}},effortLevels.map(e=>i.createElement("option",{key:e,value:e},e))))';

if (!source.includes(before)) {
  console.error("Target snippet not found or already patched:", target);
  process.exit(2);
}

const patched = source.replace(before, after);
fs.writeFileSync(target, patched);
console.log("Patched:", target);
