import { mkdir, readFile, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import path from "node:path";

const NOTES_FILE = "access-notes.json";

export function defaultConfigDir() {
  if (process.env.BLAKSYNC_CONFIG_DIR) return path.resolve(process.env.BLAKSYNC_CONFIG_DIR);
  return path.join(homedir(), ".config", "blaksync");
}

export class AccessNotesStore {
  constructor(configDir = defaultConfigDir()) {
    this.configDir = path.resolve(configDir);
    this.filePath = path.join(this.configDir, NOTES_FILE);
  }

  async ensureDir() {
    await mkdir(this.configDir, { recursive: true, mode: 0o700 });
  }

  async readAll() {
    try {
      const raw = await readFile(this.filePath, "utf8");
      const data = JSON.parse(raw);
      return data.folders && typeof data.folders === "object" ? data.folders : {};
    } catch (error) {
      if (error && error.code === "ENOENT") return {};
      throw error;
    }
  }

  async get(folderId) {
    const folders = await this.readAll();
    return folders[folderId] ?? "";
  }

  async set(folderId, note) {
    if (!folderId) throw new Error("Folder ID is required");
    await this.ensureDir();
    const folders = await this.readAll();
    const trimmed = String(note ?? "").trim();
    if (trimmed) folders[folderId] = trimmed;
    else delete folders[folderId];
    await writeFile(this.filePath, `${JSON.stringify({ folders }, null, 2)}\n`, { mode: 0o600 });
    return folders[folderId] ?? "";
  }

  async remove(folderId) {
    return this.set(folderId, "");
  }
}
