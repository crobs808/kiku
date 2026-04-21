#!/usr/bin/env node

import { execSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const args = new Set(process.argv.slice(2));
const apply = args.has("--apply");

const repoRoot = run("git rev-parse --show-toplevel");
process.chdir(repoRoot);

const TAURI_CONF_PATH = path.join(repoRoot, "apps/desktop/src-tauri/tauri.conf.json");
const DESKTOP_PACKAGE_PATH = path.join(repoRoot, "apps/desktop/package.json");
const WORKSPACE_CARGO_PATH = path.join(repoRoot, "Cargo.toml");

const versionTrackedFiles = [
  "apps/desktop/src-tauri/tauri.conf.json",
  "apps/desktop/package.json",
  "Cargo.toml"
];

const currentVersion = parseSemver(readJsonVersion(TAURI_CONF_PATH));
if (!currentVersion) {
  fail(`Could not parse current version from ${TAURI_CONF_PATH}`);
}

const latestReleaseTag = getLatestSemverTag();
const taggedVersion = latestReleaseTag ? parseSemver(latestReleaseTag.slice(1)) : null;

const baselineRef = latestReleaseTag ?? getLastVersionChangeCommit(versionTrackedFiles);
const commitRange = baselineRef ? `${baselineRef}..HEAD` : "HEAD";
const commits = getCommitsInRange(commitRange);

const bumpType =
  commits.length === 0
    ? "patch"
    : latestReleaseTag === null && baselineRef === null
      ? "patch"
      : determineBumpType(commits);

const baseVersion = maxVersion(currentVersion, taggedVersion);
const nextVersion = incrementVersion(baseVersion, bumpType);
const nextVersionString = formatSemver(nextVersion);

if (apply) {
  writeJsonVersion(TAURI_CONF_PATH, nextVersionString);
  writeJsonVersion(DESKTOP_PACKAGE_PATH, nextVersionString);
  writeWorkspaceCargoVersion(WORKSPACE_CARGO_PATH, nextVersionString);

  console.log(`Applied version ${nextVersionString} (${bumpType})`);
  if (latestReleaseTag) {
    console.log(`Release baseline tag: ${latestReleaseTag}`);
  } else if (baselineRef) {
    console.log(`Release baseline commit: ${baselineRef}`);
  } else {
    console.log("Release baseline: none");
  }
} else {
  console.log(nextVersionString);
}

function run(command) {
  return execSync(command, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }).trim();
}

function runMaybe(command) {
  try {
    return run(command);
  } catch {
    return "";
  }
}

function fail(message) {
  console.error(message);
  process.exit(1);
}

function parseSemver(value) {
  if (typeof value !== "string") {
    return null;
  }

  const match = value.trim().match(/^(\d+)\.(\d+)\.(\d+)$/);
  if (!match) {
    return null;
  }

  return {
    major: Number.parseInt(match[1], 10),
    minor: Number.parseInt(match[2], 10),
    patch: Number.parseInt(match[3], 10)
  };
}

function formatSemver(version) {
  return `${version.major}.${version.minor}.${version.patch}`;
}

function compareSemver(left, right) {
  if (left.major !== right.major) {
    return left.major - right.major;
  }
  if (left.minor !== right.minor) {
    return left.minor - right.minor;
  }
  return left.patch - right.patch;
}

function maxVersion(left, right) {
  if (!right) {
    return left;
  }
  return compareSemver(left, right) >= 0 ? left : right;
}

function incrementVersion(version, bumpType) {
  if (bumpType === "major") {
    return { major: version.major + 1, minor: 0, patch: 0 };
  }
  if (bumpType === "minor") {
    return { major: version.major, minor: version.minor + 1, patch: 0 };
  }
  return { major: version.major, minor: version.minor, patch: version.patch + 1 };
}

function readJsonVersion(filePath) {
  const parsed = JSON.parse(fs.readFileSync(filePath, "utf8"));
  return parsed.version;
}

function writeJsonVersion(filePath, version) {
  const parsed = JSON.parse(fs.readFileSync(filePath, "utf8"));
  parsed.version = version;
  fs.writeFileSync(filePath, `${JSON.stringify(parsed, null, 2)}\n`);
}

function writeWorkspaceCargoVersion(filePath, version) {
  const source = fs.readFileSync(filePath, "utf8");
  const lines = source.split(/\r?\n/);
  const hasFinalNewline = source.endsWith("\n");
  let inWorkspacePackage = false;
  let replaced = false;

  for (let idx = 0; idx < lines.length; idx += 1) {
    const line = lines[idx];
    const trimmed = line.trim();

    if (/^\[[^\]]+\]$/.test(trimmed)) {
      inWorkspacePackage = trimmed === "[workspace.package]";
      continue;
    }

    if (inWorkspacePackage && /^version\s*=/.test(trimmed)) {
      lines[idx] = `version = "${version}"`;
      replaced = true;
      break;
    }
  }

  if (!replaced) {
    fail(`Could not find [workspace.package] version field in ${filePath}`);
  }

  const next = lines.join("\n") + (hasFinalNewline ? "\n" : "");
  fs.writeFileSync(filePath, next);
}

function getLatestSemverTag() {
  const output = runMaybe('git tag --sort=-version:refname --list "v[0-9]*.[0-9]*.[0-9]*"');
  const first = output.split(/\r?\n/).map((line) => line.trim()).find(Boolean);
  return first ?? null;
}

function getLastVersionChangeCommit(files) {
  const quotedPaths = files.map((filePath) => `'${filePath.replace(/'/g, "'\\''")}'`).join(" ");
  const output = runMaybe(`git log -n 1 --format=%H -- ${quotedPaths}`);
  return output || null;
}

function getCommitsInRange(rangeSpec) {
  const output = runMaybe(`git log --format=%s%x1f%b%x1e ${rangeSpec}`);
  if (!output) {
    return [];
  }

  return output
    .split("\x1e")
    .map((entry) => entry.trim())
    .filter(Boolean)
    .map((entry) => {
      const [subject = "", body = ""] = entry.split("\x1f");
      return {
        subject: subject.trim(),
        body: body.trim()
      };
    });
}

function determineBumpType(commits) {
  for (const commit of commits) {
    const isBreakingSubject = /^[a-zA-Z]+(?:\([^)]+\))?!:/.test(commit.subject);
    const hasBreakingBody = /(^|\n)BREAKING CHANGE:/m.test(commit.body);
    if (isBreakingSubject || hasBreakingBody) {
      return "major";
    }
  }

  for (const commit of commits) {
    if (/^feat(?:\([^)]+\))?:/i.test(commit.subject)) {
      return "minor";
    }
  }

  return "patch";
}
