import express from "express";
import { execFile } from "child_process";
import fs from "fs";
import path from "path";
import net from "net";

const app = express();
app.set("trust proxy", true);

function testIP(setName, ip) {
  return new Promise((resolve) => {
    execFile("ipset", ["test", setName, ip], (err, stdout, stderr) => {
      const output = (stdout + stderr).trim();
      // clean up buffers
      if (stdout) stdout = "";
      if (stderr) stderr = "";
      if (err && err.message) err.message = "";

      if (output.includes("is in set")) resolve(true);
      else if (output.includes("is NOT in set")) resolve(false);
      else resolve(false);
    });
  });
}

const STATS_FILE = path.resolve(process.cwd(), "redblock-stats.json");

function loadStats() {
  try {
    const raw = fs.readFileSync(STATS_FILE, "utf8");
    const parsed = JSON.parse(raw);
    return {
      requests: Number(parsed.requests) || 0,
      blocks: Number(parsed.blocks) || 0,
      passes: Number(parsed.passes) || 0,
    };
  } catch (e) {
    return { requests: 0, blocks: 0, passes: 0 };
  }
}

function saveStats(stats) {
  try {
    const tmp = STATS_FILE + ".tmp";
    fs.writeFileSync(tmp, JSON.stringify(stats), { encoding: "utf8" });
    fs.renameSync(tmp, STATS_FILE);
  } catch (e) {
    console.error("Failed to save stats:", e?.message ?? e);
  }
}

let stats = loadStats();

app.use((req, res, next) => {
  res.header("Access-Control-Allow-Origin", "*");
  res.header("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS");
  res.header("Access-Control-Allow-Headers", "Origin, X-Requested-With, Content-Type, Accept, Authorization");
  next();
});

app.get("/", async (req, res) => {
  res.redirect("https://dispherical.com/tools/redblock");
});

app.get("/test", async (req, res) => {
  let ip = req.query.ip;

  if (!ip) return res.status(400).json({ error: "missing ?ip=" });
  if (!net.isIP(ip)) return res.status(400).json({ error: "invalid ip" });

  const setName = ip.includes(":") ? "blocked6" : "blocked4";
  const blocked = await testIP(setName, ip);

  try {
    stats.requests = (stats.requests || 0) + 1;
    if (blocked) stats.blocks = (stats.blocks || 0) + 1;
    else stats.passes = (stats.passes || 0) + 1;
    saveStats(stats);
  } catch (e) {
    console.error("Failed to update stats:", e?.message ?? e);
  }

  res.json({ blocked });

  // memory sanitation
  try {
    if (typeof ip === "string") {
      const buf = Buffer.from(ip);
      buf.fill(0);
    }
    ip = null;
    req.query.ip = undefined;
  } catch (e) {
  }
});

app.get("/stats", (req, res) => {
  const s = loadStats();
  const body = `Requests: ${s.requests}\nBlocks: ${s.blocks}\nPasses: ${s.passes}\n`;
  res.set("Content-Type", "text/plain; charset=utf-8");
  res.send(body);
});

app.listen(8080, () => {
  console.log("Redblock API listening on port 8080");
});
