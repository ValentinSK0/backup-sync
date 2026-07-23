// Vyrenderuje app-icon.svg do app-icon.png (1024x1024).
// Pouzitie:  npm i --no-save sharp && node scripts/render-icon.mjs && npx tauri icon app-icon.png
import sharp from "sharp";

await sharp("app-icon.svg", { density: 96 })
  .resize(1024, 1024)
  .png()
  .toFile("app-icon.png");

console.log("app-icon.png hotovy");
