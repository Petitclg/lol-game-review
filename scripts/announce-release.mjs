// Posts the release notes of a tag to Discord.
//
//   node scripts/announce-release.mjs v0.2.0 [--dry-run]
//
// Reads DISCORD_WEBHOOK_URL (secret), GITHUB_REPOSITORY and GITHUB_SERVER_URL from the
// environment. Without the webhook the announcement is skipped, not failed.

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { announce } from "./release-notes.mjs";

const args = process.argv.slice(2);
const dryRun = args.includes("--dry-run");
const tag = args.find((a) => !a.startsWith("--")) ?? process.env.TAG ?? "";

try {
  await announce({
    tag,
    env: process.env,
    dryRun,
    git: (gitArgs) => execFileSync("git", gitArgs, { encoding: "utf8" }),
    readFile: (path) => (existsSync(path) ? readFileSync(path, "utf8") : null),
  });
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  process.exitCode = 1;
}
