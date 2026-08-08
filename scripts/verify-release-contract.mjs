import { readFileSync, statSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { parse } from "yaml";

const semver = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*)(?:\.(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*))*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/;
const validSemVers = ["0.1.0", "1.0.0-alpha.1", "1.0.0+build.1"];
const invalidSemVers = ["01.0.0", "1.0.0-01", "1.0.0-alpha..1", "1.0.0+build..1"];
const inputErrorMessage = "release contract input error: unable to read or parse release inputs";
const privateSecurityAdvisoryUrl = "https://github.com/WiseZenn/PureWall/security/advisories/new";
const releasesUrl = "https://github.com/WiseZenn/PureWall/releases";
const issueFormPaths = [
  ".github/ISSUE_TEMPLATE/bug.yml",
  ".github/ISSUE_TEMPLATE/feature.yml",
  ".github/ISSUE_TEMPLATE/safety.yml",
  ".github/ISSUE_TEMPLATE/config.yml",
];
const requiredPublicFiles = [
  ...issueFormPaths,
  ".github/pull_request_template.md",
  "SUPPORT.md",
  "ROADMAP.md",
  "docs/REPOSITORY_POLICY.md",
];
const requiredPublicContent = new Map([
  ["README.md", [["PureWall Releases link", releasesUrl]]],
  [
    "SECURITY.md",
    [
      ["private security advisory link", privateSecurityAdvisoryUrl],
      ["private reporting release prerequisite", "mandatory release prerequisite"],
      ["blocked release boundary", "release process is **BLOCKED**"],
    ],
  ],
]);
const forbiddenPublicContent = new Map([
  ["SECURITY.md", [["unverifiable GitHub profile fallback", "GitHub profile"]]],
]);
const requiredPublicContractFiles = [
  ...new Set([...requiredPublicFiles, ...requiredPublicContent.keys()]),
];

function loadReleaseInputs(readText = readFileSync) {
  try {
    return {
      value: {
        packageJson: JSON.parse(readText("package.json", "utf8")),
        tauriConfig: JSON.parse(readText("src-tauri/tauri.conf.json", "utf8")),
        cargoToml: readText("src-tauri/Cargo.toml", "utf8"),
      },
    };
  } catch {
    return { error: inputErrorMessage };
  }
}

function validateUpdaterConfig(config) {
  const updaterFailures = [];
  const updater = config.plugins?.updater;

  if (updater) {
    if (config.bundle?.createUpdaterArtifacts !== true) updaterFailures.push("updater artifacts are not enabled");
    if (!Array.isArray(updater.endpoints) || updater.endpoints.some((url) => typeof url !== "string" || !url.startsWith("https://"))) updaterFailures.push("updater endpoints must use HTTPS");
    if (updater.dangerousInsecureTransportProtocol === true) updaterFailures.push("insecure updater transport is forbidden");
  }

  return updaterFailures;
}

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function containsText(value, expected) {
  if (typeof value === "string") return value.includes(expected);
  if (Array.isArray(value)) return value.some((item) => containsText(item, expected));
  if (isRecord(value)) return Object.values(value).some((item) => containsText(item, expected));
  return false;
}

function validateIssueForm(path, content) {
  let form;
  try {
    form = parse(content, { uniqueKeys: true });
  } catch {
    return [`public repository file contains invalid YAML: ${path}`];
  }

  if (!isRecord(form)) {
    return [`public repository issue form must be a top-level object: ${path}`];
  }

  if (path.endsWith("/config.yml")) {
    const failures = [];
    if (form.blank_issues_enabled !== false) {
      failures.push(`public repository issue config blank_issues_enabled must be false: ${path}`);
    }
    if (!Array.isArray(form.contact_links)) {
      failures.push(`public repository issue config contact_links must be an array: ${path}`);
    }
    return failures;
  }

  const failures = [];
  for (const field of ["name", "description"]) {
    if (typeof form[field] !== "string" || form[field].trim().length === 0) {
      failures.push(`public repository issue form ${field} must be a non-empty string: ${path}`);
    }
  }
  if (!Array.isArray(form.body) || form.body.length === 0) {
    failures.push(`public repository issue form body must be a non-empty array: ${path}`);
    return failures;
  }

  if (path.endsWith("/bug.yml")) {
    if (!isRecord(form.body[0]) || form.body[0].type !== "markdown") {
      failures.push(`public repository bug form body[0] must be markdown: ${path}`);
    } else if (!containsText(form.body[0], privateSecurityAdvisoryUrl)) {
      failures.push(`public repository bug form body[0] must contain the private security advisory link: ${path}`);
    }
  }
  if (path.endsWith("/safety.yml") && !containsText(form.body, privateSecurityAdvisoryUrl)) {
    failures.push(`public repository safety form must contain the private security advisory link: ${path}`);
  }

  return failures;
}

function validatePublicRepositoryFiles(
  paths = requiredPublicContractFiles,
  { statPath = statSync, readText = readFileSync } = {},
) {
  const publicFailures = [];

  for (const path of paths) {
    let status;
    try {
      status = statPath(path);
    } catch {
      publicFailures.push(`missing public repository file: ${path}`);
      continue;
    }

    if (typeof status.isFile !== "function" || !status.isFile()) {
      publicFailures.push(`public repository path is not a regular file: ${path}`);
      continue;
    }
    if (typeof status.size !== "number" || status.size <= 0) {
      publicFailures.push(`public repository file is empty: ${path}`);
      continue;
    }

    let content;
    try {
      content = readText(path, "utf8");
    } catch {
      publicFailures.push(`unable to read public repository file: ${path}`);
      continue;
    }
    if (typeof content !== "string" || content.trim().length === 0) {
      publicFailures.push(`public repository file is empty: ${path}`);
      continue;
    }

    if (issueFormPaths.includes(path)) {
      publicFailures.push(...validateIssueForm(path, content));
    }

    for (const [label, marker] of requiredPublicContent.get(path) ?? []) {
      const present = marker instanceof RegExp ? marker.test(content) : content.includes(marker);
      if (!present) {
        publicFailures.push(`public repository file missing ${label}: ${path}`);
      }
    }
    for (const [label, marker] of forbiddenPublicContent.get(path) ?? []) {
      const present = marker instanceof RegExp ? marker.test(content) : content.includes(marker);
      if (present) {
        publicFailures.push(`public repository file contains ${label}: ${path}`);
      }
    }
  }

  return publicFailures;
}

function validateGitIgnoreScope(runGit = spawnSync) {
  const probePaths = ["output/generated.ts", "src/output/generated.ts"];
  const result = runGit("git", ["check-ignore", "--no-index", "--stdin"], {
    encoding: "utf8",
    input: `${probePaths.join("\n")}\n`,
  });
  if (result.error || ![0, 1].includes(result.status)) {
    return ["unable to execute git ignore scope probe"];
  }

  const ignoredPaths = new Set(
    result.stdout
      .split(/\r?\n/)
      .map((path) => path.trim().replaceAll("\\", "/").replace(/^\.\//, ""))
      .filter(Boolean),
  );
  const ignoreFailures = [];
  if (!ignoredPaths.has("output/generated.ts")) {
    ignoreFailures.push("root output directory is not ignored");
  }
  if (ignoredPaths.has("src/output/generated.ts")) {
    ignoreFailures.push("nested src/output directory is incorrectly ignored");
  }
  return ignoreFailures;
}

const releaseWorkflowPath = ".github/workflows/release.yml";

const requiredReleaseWorkflowMarkers = [
  ["tag trigger", "tags:"],
  ["release tag validation", "PUREWALL_RELEASE_TAG"],
  ["updater signing key", "TAURI_SIGNING_PRIVATE_KEY"],
  ["updater public key", "TAURI_SIGNING_PUBLIC_KEY"],
  ["Authenticode certificate", "WINDOWS_CERTIFICATE"],
  ["draft release", "releaseDraft: true"],
  ["artifact attestation", "actions/attest@v4"],
  ["checksum file", "SHA256SUMS.txt"],
];

function loadWorkflowContract(path, { statPath = statSync, readText = readFileSync, kind = "release workflow" } = {}) {
  try {
    const status = statPath(path);
    if (typeof status.isFile !== "function" || !status.isFile()) {
      return { error: `${kind} path is not a regular file: ${path}` };
    }
    const content = readText(path, "utf8");
    if (typeof content !== "string" || content.trim().length === 0) {
      return { error: `${kind} file is empty: ${path}` };
    }
    return { content };
  } catch {
    return { error: `missing ${kind} file: ${path}` };
  }
}

function validateWorkflowMarkers(path, requiredMarkers, forbiddenMarkers, opts = {}) {
  const loaded = loadWorkflowContract(path, opts);
  if (loaded.error) return [loaded.error];
  const failures = [];
  for (const [label, marker] of requiredMarkers) {
    if (!loaded.content.includes(marker)) failures.push(`${path} missing ${label} marker: ${marker}`);
  }
  const lowerContent = loaded.content.toLowerCase();
  for (const [label, marker] of forbiddenMarkers) {
    if (lowerContent.includes(marker.toLowerCase())) failures.push(`${path} contains forbidden ${label}: ${marker}`);
  }
  return failures;
}

function validateReleaseWorkflow(opts = {}) {
  return validateWorkflowMarkers(releaseWorkflowPath, requiredReleaseWorkflowMarkers, [], {
    ...opts,
    kind: "release workflow",
  });
}

const installerSmokeWorkflowPath = ".github/workflows/installer-smoke.yml";

const requiredInstallerSmokeMarkers = [
  ["manual dispatch", "workflow_dispatch"],
  ["disposable runner guard", "-DisposableRunner"],
  ["checksum file", "SHA256SUMS.txt"],
  ["attestation verification", "gh attestation verify"],
];

const forbiddenInstallerSmokeMarkers = [
  ["recursive removal", "Remove-Item -Recurse"],
  ["HKLM registry scope", "HKLM"],
  ["Windows policy registry scope", "Policies"],
  ["Windows 11 context-menu CLSID", "86ca1aa0-34aa-4e8b-a509-50c905bae2a2"],
];

function validateInstallerSmokeWorkflow(opts = {}) {
  return validateWorkflowMarkers(installerSmokeWorkflowPath, requiredInstallerSmokeMarkers, forbiddenInstallerSmokeMarkers, {
    ...opts,
    kind: "installer smoke workflow",
  });
}

function runContractSelfTest() {
  const selfTestFailures = [
    ...validSemVers.filter((version) => !semver.test(version)).map((version) => `rejected valid SemVer: ${version}`),
    ...invalidSemVers.filter((version) => semver.test(version)).map((version) => `accepted invalid SemVer: ${version}`),
  ];
  const endpointFailures = validateUpdaterConfig({
    bundle: { createUpdaterArtifacts: true },
    plugins: { updater: { endpoints: [42] } },
  });
  if (!endpointFailures.includes("updater endpoints must use HTTPS")) {
    selfTestFailures.push("accepted non-string updater endpoint");
  }
  const inputErrorResult = loadReleaseInputs(() => {
    throw new Error("C:\\Users\\example\\private-release-input.json");
  });
  if (inputErrorResult.error !== inputErrorMessage) {
    selfTestFailures.push("input failure did not use the stable release contract error");
  }

  const directoryFailures = validatePublicRepositoryFiles(["SUPPORT.md"], {
    statPath: () => ({ isFile: () => false, size: 20 }),
    readText: () => "support",
  });
  if (!directoryFailures.some((failure) => failure.includes("not a regular file"))) {
    selfTestFailures.push("public repository validator accepted a directory");
  }

  const emptyFailures = validatePublicRepositoryFiles(["ROADMAP.md"], {
    statPath: () => ({ isFile: () => true, size: 0 }),
    readText: () => "",
  });
  if (!emptyFailures.some((failure) => failure.includes("empty"))) {
    selfTestFailures.push("public repository validator accepted an empty file");
  }

  const brokenIssueFailures = validatePublicRepositoryFiles(
    issueFormPaths,
    {
      statPath: () => ({ isFile: () => true, size: 12 }),
      readText: () => "name: broken\n",
    },
  );
  for (const path of issueFormPaths) {
    if (!brokenIssueFailures.some((failure) => failure.includes(path))) {
      selfTestFailures.push(`public repository validator accepted broken ${path}`);
    }
  }

  const issueFormStat = () => ({ isFile: () => true, size: 512 });
  const malformedIssueForms = [
    {
      label: "an unclosed flow collection",
      path: ".github/ISSUE_TEMPLATE/config.yml",
      expectedFailure: "invalid YAML",
      content: `blank_issues_enabled: false
contact_links:
  links: [
    SUPPORT.md
    SECURITY.md
`,
    },
    {
      label: "a duplicate top-level key",
      path: ".github/ISSUE_TEMPLATE/bug.yml",
      expectedFailure: "invalid YAML",
      content: `name: Bug report
name: Duplicate bug report
description: Synthetic duplicate-key fixture.
body:
  - type: markdown
    attributes:
      value: |
        ${privateSecurityAdvisoryUrl}
        Windows version
        PureWall version or commit
        Reproduction steps
        Expected behavior
        Actual behavior
`,
    },
    {
      label: "a non-array body",
      path: ".github/ISSUE_TEMPLATE/feature.yml",
      expectedFailure: "body must be a non-empty array",
      content: `name: Feature request
description: Synthetic wrong-nesting fixture.
body:
  type: checkboxes
  attributes:
    label: Existing PureWall workflow
    description: PureWall-X stays deferred and this does not add an online feed.
`,
    },
  ];
  for (const { label, path, expectedFailure, content } of malformedIssueForms) {
    const malformedFailures = validatePublicRepositoryFiles([path], {
      statPath: issueFormStat,
      readText: () => content,
    });
    if (!malformedFailures.some((failure) => failure.includes(expectedFailure))) {
      selfTestFailures.push(`public repository validator accepted ${label}`);
    }
  }

  const flexibleIndentationFailures = validatePublicRepositoryFiles(
    [".github/ISSUE_TEMPLATE/safety.yml"],
    {
      statPath: issueFormStat,
      readText: () => `name: Safety boundary question
description: Synthetic valid indentation fixture.
body:
  # Comments may appear before the body sequence.
    - type: markdown
      attributes:
        value: |
          Use ${privateSecurityAdvisoryUrl} for destructive file operations,
          registry writes, installer or uninstaller behavior, path traversal,
          and updater/signature verification.
`,
    },
  );
  if (flexibleIndentationFailures.length > 0) {
    selfTestFailures.push("public repository validator rejected valid flexible YAML indentation");
  }

  const misScopedIgnoreFailures = validateGitIgnoreScope(() => ({
    status: 0,
    stdout: "output/generated.ts\nsrc/output/generated.ts\n",
  }));
  if (!misScopedIgnoreFailures.includes("nested src/output directory is incorrectly ignored")) {
    selfTestFailures.push("git ignore validator accepted a nested src/output match");
  }

  const releaseWorkflowStat = () => ({ isFile: () => true, size: 4096 });
  const releaseWorkflowFixture = `name: release
on:
  push:
    tags:
      - "v*"
  workflow_dispatch:
jobs:
  release:
    runs-on: windows-latest
    steps:
      - run: echo "PUREWALL_RELEASE_TAG"
      - run: echo "TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PUBLIC_KEY WINDOWS_CERTIFICATE"
      - uses: tauri-apps/tauri-action@v1
        with:
          releaseDraft: true
      - uses: actions/attest@v4
      - run: echo "SHA256SUMS.txt"
`;

  const missingWorkflowFailures = validateReleaseWorkflow({
    statPath: () => {
      throw new Error("C:\\repo\\.github\\workflows\\release.yml");
    },
  });
  if (!missingWorkflowFailures.some((failure) => failure.includes("missing release workflow file"))) {
    selfTestFailures.push("release workflow validator accepted a missing release.yml");
  }
  const emptyWorkflowFailures = validateReleaseWorkflow({
    statPath: releaseWorkflowStat,
    readText: () => "",
  });
  if (!emptyWorkflowFailures.some((failure) => failure.includes("empty"))) {
    selfTestFailures.push("release workflow validator accepted an empty release.yml");
  }
  const markerGapFailures = validateReleaseWorkflow({
    statPath: releaseWorkflowStat,
    readText: () => `name: release\non: {}\n`,
  });
  if (!markerGapFailures.some((failure) => failure.includes("missing tag trigger marker"))) {
    selfTestFailures.push("release workflow validator accepted a workflow without the tag trigger");
  }
  const completeWorkflowFailures = validateReleaseWorkflow({
    statPath: releaseWorkflowStat,
    readText: () => releaseWorkflowFixture,
  });
  if (completeWorkflowFailures.length > 0) {
    selfTestFailures.push("release workflow validator rejected a complete release workflow");
  }

  const smokeWorkflowFixture = `name: installer-smoke
on:
  workflow_dispatch:
    inputs:
      currentTag:
        required: true
jobs:
  smoke:
    runs-on: windows-latest
    steps:
      - run: gh release download --pattern "SHA256SUMS.txt"
      - run: gh attestation verify artifact.msi --repo WiseZenn/PureWall
      - run: powershell -File scripts/verify-install-lifecycle.ps1 -DisposableRunner
`;

  const missingSmokeFailures = validateInstallerSmokeWorkflow({
    statPath: () => {
      throw new Error("C:\\repo\\.github\\workflows\\installer-smoke.yml");
    },
  });
  if (!missingSmokeFailures.some((failure) => failure.includes("missing installer smoke workflow file"))) {
    selfTestFailures.push("installer smoke validator accepted a missing installer-smoke.yml");
  }
  const incompleteSmokeFailures = validateInstallerSmokeWorkflow({
    statPath: releaseWorkflowStat,
    readText: () => `name: installer-smoke\non: {}\n`,
  });
  if (!incompleteSmokeFailures.some((failure) => failure.includes("missing manual dispatch marker"))) {
    selfTestFailures.push("installer smoke validator accepted a workflow without workflow_dispatch");
  }
  const forbiddenSmokeFailures = validateInstallerSmokeWorkflow({
    statPath: releaseWorkflowStat,
    readText: () => `${smokeWorkflowFixture}
      - run: Remove-Item -Recurse "$env:RUNNER_TEMP"
      - run: echo HKLM
      - run: echo Policies
      - run: echo 86ca1aa0-34aa-4e8b-a509-50c905bae2a2
`,
  });
  for (const [label] of forbiddenInstallerSmokeMarkers) {
    if (!forbiddenSmokeFailures.some((failure) => failure.includes(`forbidden ${label}`))) {
      selfTestFailures.push(`installer smoke validator accepted ${label} text`);
    }
  }
  const completeSmokeFailures = validateInstallerSmokeWorkflow({
    statPath: releaseWorkflowStat,
    readText: () => smokeWorkflowFixture,
  });
  if (completeSmokeFailures.length > 0) {
    selfTestFailures.push("installer smoke validator rejected a complete smoke workflow");
  }

  selfTestFailures.push(...validatePublicRepositoryFiles());
  selfTestFailures.push(...validateGitIgnoreScope());
  selfTestFailures.push(...validateReleaseWorkflow());
  selfTestFailures.push(...validateInstallerSmokeWorkflow());

  if (selfTestFailures.length) {
    console.error(selfTestFailures.join("\n"));
    process.exit(1);
  }

  console.log(
    `Release contract self-test OK (strict SemVer: ${validSemVers.length} valid, ${invalidSemVers.length} invalid; structural YAML, public repository files, ignore scope, and privileged workflow contract)`,
  );
}

if (process.argv.includes("--self-test")) {
  runContractSelfTest();
  process.exit(0);
}

const inputResult = loadReleaseInputs();
if (inputResult.error) {
  console.error(inputResult.error);
  process.exit(1);
}

const { packageJson, tauriConfig, cargoToml } = inputResult.value;
const cargoVersion = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
const versions = [packageJson.version, cargoVersion, tauriConfig.version];
const failures = [];

failures.push(...validatePublicRepositoryFiles());
failures.push(...validateGitIgnoreScope());
failures.push(...validateReleaseWorkflow());
failures.push(...validateInstallerSmokeWorkflow());

if (versions.some((version) => version !== versions[0])) failures.push(`version mismatch: ${versions.join(", ")}`);
if (!semver.test(versions[0] ?? "")) failures.push(`invalid SemVer: ${versions[0]}`);

const tag = process.env.PUREWALL_RELEASE_TAG;
if (tag && tag !== `v${versions[0]}`) failures.push(`tag ${tag} does not match v${versions[0]}`);

failures.push(...validateUpdaterConfig(tauriConfig));

if (failures.length) {
  console.error(failures.join("\n"));
  process.exit(1);
}
console.log(`PureWall release contract OK for ${versions[0]}`);
