import { performance } from "node:perf_hooks";
import fs from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

const pkgDir = path.resolve(process.argv[2] ?? "neco-nostr-wasm/pkg");
const prefix = process.argv[3] ?? "q";
const attempts = Number(process.argv[4] ?? "20000");
const iterations = Number(process.argv[5] ?? "3");
const topK = Number(process.argv[6] ?? "5");

const mod = await import(pathToFileURL(path.join(pkgDir, "neco_nostr_wasm.js")).href);
const wasmBytes = await fs.readFile(path.join(pkgDir, "neco_nostr_wasm_bg.wasm"));
await mod.default({ module_or_path: wasmBytes });

let vanityTotalMs = 0;
let vanitySuccess = 0;
let lastVanityBestMatch = 0;

for (let i = 0; i < iterations; i += 1) {
  const start = performance.now();
  try {
    const vanity = JSON.parse(mod.mine_vanity_batch(prefix, attempts));
    lastVanityBestMatch = vanity.npub?.slice(5).startsWith(prefix) ? prefix.length : 0;
    vanitySuccess += 1;
  } catch {
    // Exhausted attempts still contributes to elapsed time.
  }
  vanityTotalMs += performance.now() - start;
}

let candidatesTotalMs = 0;
let lastCandidateCount = 0;
let lastCandidateBestMatch = 0;

for (let i = 0; i < iterations; i += 1) {
  const start = performance.now();
  const candidates = JSON.parse(mod.mine_vanity_with_candidates(prefix, attempts, topK));
  lastCandidateCount = candidates.length;
  lastCandidateBestMatch = candidates[0]?.matched_len ?? 0;
  candidatesTotalMs += performance.now() - start;
}

console.log(
  JSON.stringify(
    {
      bench: "neco-nostr-wasm vanity measure",
      pkgDir,
      prefix,
      attempts,
      iterations,
      top_k: topK,
      vanity_batch_avg_ms: Number((vanityTotalMs / iterations).toFixed(3)),
      vanity_batch_successes: vanitySuccess,
      vanity_batch_best_match: lastVanityBestMatch,
      vanity_batch_attempts_per_ms: Number((attempts / (vanityTotalMs / iterations)).toFixed(3)),
      vanity_candidates_avg_ms: Number((candidatesTotalMs / iterations).toFixed(3)),
      vanity_candidates_count: lastCandidateCount,
      vanity_candidates_best_match: lastCandidateBestMatch,
      vanity_candidates_attempts_per_ms: Number(
        (attempts / (candidatesTotalMs / iterations)).toFixed(3),
      ),
    },
    null,
    2,
  ),
);
