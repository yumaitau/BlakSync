import { constants } from "node:fs";
import { access, mkdir } from "node:fs/promises";
import { spawn } from "node:child_process";
import path from "node:path";
import { defaultConfigDir } from "./access-notes.js";
import { assertDeviceId } from "./syncthing.js";

const GUI_ADDRESS = "127.0.0.1:8384";

export async function launchSyncthing({
  binary = process.env.BLAKSYNC_SYNCTHING || "syncthing",
  home = defaultConfigDir(),
  runCommand = runSyncthingCommand,
  serve = serveSyncthing,
  logger = console.log,
  pathExists = fileExists,
  mkdirImpl = mkdir,
} = {}) {
  if (!binary) throw new Error("A Syncthing executable is required");
  const resolvedHome = path.resolve(home);
  await mkdirImpl(resolvedHome, { recursive: true, mode: 0o700 });

  if (!(await pathExists(path.join(resolvedHome, "config.xml")))) {
    await runCommand(binary, ["generate", `--home=${resolvedHome}`, "--no-port-probing"]);
  }

  const deviceId = (await runCommand(binary, ["device-id", `--home=${resolvedHome}`])).trim();
  assertDeviceId(deviceId);

  logger(`Syncthing GUI: http://${GUI_ADDRESS}`);
  logger(`Device ID: ${deviceId}`);
  logger(`BlakSync config: ${resolvedHome}`);

  const exitCode = await serve(
    binary,
    ["serve", `--home=${resolvedHome}`, `--gui-address=${GUI_ADDRESS}`, "--no-browser", "--no-port-probing"],
    { ...process.env, STVERSIONEXTRA: "BlakSync" },
  );
  return { deviceId, guiUrl: `http://${GUI_ADDRESS}`, home: resolvedHome, exitCode };
}

export function runSyncthingCommand(binary, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(binary, args, { stdio: ["ignore", "pipe", "pipe"] });
    const stdout = [];
    const stderr = [];
    child.stdout.on("data", (chunk) => stdout.push(chunk));
    child.stderr.on("data", (chunk) => stderr.push(chunk));
    child.once("error", (error) => reject(executableError(binary, error)));
    child.once("close", (code) => {
      if (code === 0) {
        resolve(Buffer.concat(stdout).toString("utf8"));
        return;
      }
      const detail = Buffer.concat(stderr).toString("utf8").trim();
      reject(new Error(`Syncthing ${args[0]} failed (${code ?? "unknown"})${detail ? `: ${detail}` : ""}`));
    });
  });
}

export function serveSyncthing(binary, args, env) {
  return new Promise((resolve, reject) => {
    const child = spawn(binary, args, { stdio: "inherit", env });
    child.once("error", (error) => reject(executableError(binary, error)));
    child.once("close", (code) => resolve(code ?? 1));
  });
}

async function fileExists(filePath) {
  try {
    await access(filePath, constants.F_OK);
    return true;
  } catch (error) {
    if (error?.code === "ENOENT") return false;
    throw error;
  }
}

function executableError(binary, error) {
  if (error?.code === "ENOENT") {
    return new Error(`Syncthing executable not found: ${binary}. Install Syncthing or pass --syncthing PATH.`);
  }
  return error;
}
