#!/usr/bin/env node
// 用法: node scripts/set-version.js 3.52.0
// 同步更新 package.json / src-tauri/Cargo.toml / src-tauri/tauri.conf.json 的版本号

const fs = require("fs");
const path = require("path");

const v = process.argv[2];
if (!v || !/^\d+\.\d+\.\d+/.test(v)) {
  console.error("用法: npm run version <x.y.z>");
  process.exit(1);
}

const root = path.resolve(__dirname, "..");

// 1. package.json
const pkgPath = path.join(root, "package.json");
const pkg = JSON.parse(fs.readFileSync(pkgPath, "utf8"));
pkg.version = v;
fs.writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + "\n");

// 2. Cargo.toml
const cargoPath = path.join(root, "src-tauri", "Cargo.toml");
let cargo = fs.readFileSync(cargoPath, "utf8");
cargo = cargo.replace(/^version = "[^"]+"/m, `version = "${v}"`);
fs.writeFileSync(cargoPath, cargo);

// 3. tauri.conf.json
const tauriPath = path.join(root, "src-tauri", "tauri.conf.json");
const tauri = JSON.parse(fs.readFileSync(tauriPath, "utf8"));
tauri.version = v;
fs.writeFileSync(tauriPath, JSON.stringify(tauri, null, 2) + "\n");

console.log(`✅ 版本已同步至 ${v}`);
console.log(`   package.json`);
console.log(`   src-tauri/Cargo.toml`);
console.log(`   src-tauri/tauri.conf.json`);
