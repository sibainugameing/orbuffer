import { readFile, writeFile } from "node:fs/promises";

const input = process.argv[2];

if (!input) {
  throw new Error("release version is required, for example v0.1.0");
}

const version = input.startsWith("v") ? input.slice(1) : input;

if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(version)) {
  throw new Error(`invalid release version: ${input}`);
}

async function updateJson(path, property) {
  const document = JSON.parse(await readFile(path, "utf8"));
  document.version = version;
  await writeFile(path, `${JSON.stringify(document, null, 2)}\n`, "utf8");
  console.log(`updated ${property}: ${version}`);
}

async function updateCargoToml(path) {
  const content = await readFile(path, "utf8");
  const updated = content.replace(
    /^version = ".*"$/m,
    `version = "${version}"`,
  );

  if (updated === content) {
    throw new Error(`version field not found in ${path}`);
  }

  await writeFile(path, updated, "utf8");
  console.log(`updated ${path}: ${version}`);
}

await updateJson("package.json", "package.json");
await updateJson("src-tauri/tauri.conf.json", "tauri.conf.json");
await updateCargoToml("src-tauri/Cargo.toml");
