# PureWall Phase 1 P0 Behavior Repair Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Repair PureWall's existing P0 behavior gaps so collections, Disliked navigation, playback eligibility, command-line labeling, and minimum-window navigation match the approved product contract.

**Architecture:** Keep PureWall as one Tauri application. Add two small frontend behavior models so Vitest executes decisions instead of searching component source text, centralize the Rust playback rating rule without changing the Tauri command surface, and make the existing sidebar itself scrollable at constrained heights.

**Tech Stack:** Tauri 2, Rust, rusqlite, Vue 3, TypeScript, Pinia, Vitest, CSS, Windows WebView2.

## Global Constraints

- Scope is PureWall Phase 1 only; do not create PureWall-X code, a shared-core crate, a plugin API, or new application packages.
- PureWall remains a conventional local wallpaper manager: Import, Next, Like, Dislike, Pause, and lightweight organization are the primary loop.
- User-facing terminology is `Disliked`; internal `shortcuts`, `blacklisted`, and numeric rating identifiers may remain where renaming would add migration risk.
- Playback contract: negative ratings are ineligible, rating `1` owns two tickets, other non-negative ratings own one ticket, and selection is without replacement while enough unique candidates exist.
- Manual Next and automatic rotation must continue to reach the existing `advance_wallpaper` / `Database::get_next_wallpapers` path; Phase 2 action unification is out of scope.
- Do not add npm or Cargo dependencies.
- Do not change the SQLite schema, Tauri command names, registry implementation, autostart implementation, installer, or updater.
- Do not write HKLM, Windows policy, or the Windows 11 context-menu mode. No registry command is required for this plan.
- Preserve all pre-existing dirty working-tree changes. Never use `git add -A`, `git reset`, `git checkout --`, or destructive cleanup.
- `src/components/InspectorPanel.vue` and `src/styles.css` already contain unrelated unstaged work. Stage only the Phase 1 hunks with `git add -p`; reject every unrelated hunk. If a hunk cannot be isolated safely, leave that task uncommitted and report it rather than staging someone else's changes.
- Before the first code edit, read all of `docs/project-docs/AI_DIARY.md`, read the latest `CHANGELOG_AI.md` entry, run `cargo check`, run `npx vue-tsc --noEmit`, and confirm `DECISIONS.md` contains no relevant proposed ADR.
- After each completed task, append its scope and verification evidence to `docs/project-docs/CHANGELOG_AI.md`. Append to `AI_DIARY.md` only for a genuinely new pitfall. Do not add an ADR because this plan preserves current architecture boundaries.

---

### Task 1: Repair collection creation and expose Disliked navigation

**Files:**
- Create: `src/components/sidebarModel.ts`
- Create: `src/components/sidebarModel.test.ts`
- Modify: `src/components/Sidebar.vue:2-30`
- Modify: `src/components/Sidebar.vue:92-102`
- Modify: `src/components/Sidebar.vue:114-125`
- Modify: `docs/project-docs/CHANGELOG_AI.md`

**Interfaces:**
- Consumes: `CollectionEntry`, `FilterKey`, and `Stats` from `src/stores/wallpapers.ts`; the existing Pinia store method `createCollection(name: string, color?: string): Promise<CollectionEntry | null>`.
- Produces: `sidebarFilters`, whose definitions are rendered directly by `Sidebar.vue`; `createCollectionFromDraft(creator, draft, color): Promise<boolean>`, which calls only `creator.createCollection`.

- [ ] **Step 1: Run the required task Pre-Flight before editing code**

Run in repository root:

```powershell
Get-Content -Raw docs\project-docs\AI_DIARY.md
Get-Content docs\project-docs\CHANGELOG_AI.md -Tail 120
cd src-tauri
cargo check
cd ..
npx vue-tsc --noEmit
Select-String -Path docs\project-docs\DECISIONS.md -Pattern '(?i)proposed'
```

Expected: `cargo check` exits 0, `vue-tsc` exits 0, and the ADR search returns no relevant proposed decision. If a Windows build-cache command fails with access denied or `spawn EPERM`, use the existing approved elevated verification path from `AI_DIARY.md`; do not edit source in response to a sandbox failure.

- [ ] **Step 2: Write the failing sidebar behavior test**

Create `src/components/sidebarModel.test.ts`:

```ts
import { describe, expect, it, vi } from "vitest";
import {
  createCollectionFromDraft,
  sidebarFilters,
} from "./sidebarModel";

describe("sidebar behavior model", () => {
  it("exposes the complete library navigation with the Disliked count", () => {
    const stats = {
      total: 12,
      liked: 4,
      disliked: 3,
      blacklisted: 2,
      total_plays: 20,
    };

    expect(sidebarFilters.map(({ key, label }) => [key, label])).toEqual([
      ["all", "Library"],
      ["liked", "Liked"],
      ["disliked", "Disliked"],
      ["blacklisted", "Hidden"],
      ["tags", "Tags"],
    ]);
    expect(
      sidebarFilters.find(({ key }) => key === "disliked")?.count(stats, 0),
    ).toBe(3);
  });

  it("creates a collection through the collection API and ignores blank drafts", async () => {
    const createCollection = vi.fn(async () => ({
      id: 7,
      name: "Weekend",
      color: "#5b8def",
      wallpaper_count: 0,
    }));
    const createTag = vi.fn();
    const creator = { createCollection, createTag };

    await expect(
      createCollectionFromDraft(creator, "  Weekend  ", "#5b8def"),
    ).resolves.toBe(true);
    expect(createCollection).toHaveBeenCalledOnce();
    expect(createCollection).toHaveBeenCalledWith("Weekend", "#5b8def");
    expect(createTag).not.toHaveBeenCalled();

    await expect(
      createCollectionFromDraft(creator, "   ", "#ff7a59"),
    ).resolves.toBe(false);
    expect(createCollection).toHaveBeenCalledOnce();
  });
});
```

- [ ] **Step 3: Run the focused test and verify RED**

Run:

```powershell
npx vitest run src/components/sidebarModel.test.ts
```

Expected: FAIL because `./sidebarModel` does not exist. A zero-test run is not acceptable evidence.

- [ ] **Step 4: Implement the sidebar behavior model**

Create `src/components/sidebarModel.ts`:

```ts
import type {
  CollectionEntry,
  FilterKey,
  Stats,
} from "../stores/wallpapers";

export interface SidebarFilterDefinition {
  key: FilterKey | "tags";
  label: string;
  icon: string;
  count: (stats: Stats, tagCount: number) => number;
}

export const sidebarFilters = [
  {
    key: "all",
    label: "Library",
    icon: "library",
    count: (stats: Stats) => stats.total,
  },
  {
    key: "liked",
    label: "Liked",
    icon: "heart",
    count: (stats: Stats) => stats.liked,
  },
  {
    key: "disliked",
    label: "Disliked",
    icon: "dislike",
    count: (stats: Stats) => stats.disliked,
  },
  {
    key: "blacklisted",
    label: "Hidden",
    icon: "hidden",
    count: (stats: Stats) => stats.blacklisted,
  },
  {
    key: "tags",
    label: "Tags",
    icon: "tag",
    count: (_stats: Stats, tagCount: number) => tagCount,
  },
] satisfies readonly SidebarFilterDefinition[];

export interface CollectionCreator {
  createCollection: (
    name: string,
    color: string,
  ) => Promise<CollectionEntry | null>;
}

export async function createCollectionFromDraft(
  creator: CollectionCreator,
  draft: string,
  color: string,
): Promise<boolean> {
  const name = draft.trim();
  if (!name) return false;

  return (await creator.createCollection(name, color)) !== null;
}
```

- [ ] **Step 5: Wire `Sidebar.vue` to the tested model**

Add this import after the existing component imports:

```ts
import {
  createCollectionFromDraft,
  sidebarFilters,
} from "./sidebarModel";
```

Delete the local `const filters = [...]` block at current lines 20-30.

Replace `addCollection` with:

```ts
async function addCollection() {
  const color = collectionPalette[
    nextCollectionColor.value % collectionPalette.length
  ];
  const created = await createCollectionFromDraft(
    store,
    newCollectionName.value,
    color,
  );
  if (created) {
    newCollectionName.value = "";
    nextCollectionColor.value += 1;
  }
}
```

Replace the library navigation loop opening and count binding with:

```vue
<SidebarItem
  v-for="filter in sidebarFilters"
  :key="filter.key"
  :icon="filter.icon"
  :label="filter.label"
  :count="formatCount(filter.count(store.stats, store.tags.length))"
  :active="store.workspaceSection === 'library' && isActive(filter.key)"
  active-kind="pressed"
  @click="selectFilter(filter.key)"
/>
```

- [ ] **Step 6: Verify GREEN and type compatibility**

Run:

```powershell
npx vitest run src/components/sidebarModel.test.ts
npx vue-tsc --noEmit
```

Expected: 2 focused tests pass and `vue-tsc` exits 0.

- [ ] **Step 7: Append Task 1 process evidence**

Append this entry to `docs/project-docs/CHANGELOG_AI.md`:

```markdown
## 2026-07-19 — Phase 1 P0 behavior repairs (in progress)

### Task 1 — Sidebar collection and Disliked navigation

- Added a behavior-tested sidebar model with Library, Liked, Disliked, Hidden, and Tags navigation.
- Collection creation now calls `createCollection`; it no longer creates a tag accidentally.
- Focused Vitest: PASS, 2/2 after the missing-module RED.
- `npx vue-tsc --noEmit`: PASS.
- No registry command, system setting, SQLite schema, or wallpaper file was changed.
```

Do not update `AI_DIARY.md` unless this task discovers a new pitfall not already recorded.

- [ ] **Step 8: Review and commit only Task 1 code**

Run:

```powershell
git diff -- src/components/Sidebar.vue src/components/sidebarModel.ts src/components/sidebarModel.test.ts
git diff --check -- src/components/Sidebar.vue src/components/sidebarModel.ts src/components/sidebarModel.test.ts
git add -- src/components/Sidebar.vue src/components/sidebarModel.ts src/components/sidebarModel.test.ts
git diff --cached --check
git commit -m "fix(sidebar): repair collection and disliked navigation"
```

Expected: the commit contains only Task 1 code. Leave `CHANGELOG_AI.md` unstaged because it already contains pre-existing user changes.

---

### Task 2: Label the CLI surface as Command Line

**Files:**
- Create: `src/components/commandLineModel.ts`
- Create: `src/components/commandLineModel.test.ts`
- Modify: `src/components/Sidebar.vue:32-42`
- Modify: `src/components/InspectorPanel.vue:24-29`
- Modify: `src/components/InspectorPanel.vue:379-395`
- Modify: `docs/project-docs/CHANGELOG_AI.md`

**Interfaces:**
- Consumes: the existing internal `WorkspaceSection` value `"shortcuts"` and the existing CLI arguments `--action next|like|dislike|pause`.
- Produces: `COMMAND_LINE_TITLE`, `COMMAND_LINE_SUBTITLE`, and immutable `commandLineRows`; no Tauri or CLI contract changes.

- [ ] **Step 1: Write the failing command-line model test**

Create `src/components/commandLineModel.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import {
  COMMAND_LINE_SUBTITLE,
  COMMAND_LINE_TITLE,
  commandLineRows,
} from "./commandLineModel";

describe("command-line presentation model", () => {
  it("labels CLI actions accurately without presenting them as keyboard shortcuts", () => {
    expect(COMMAND_LINE_TITLE).toBe("Command Line");
    expect(COMMAND_LINE_SUBTITLE).toBe("Local CLI actions");
    expect(commandLineRows).toEqual([
      { label: "PW: Next", value: "--action next" },
      { label: "PW: Like", value: "--action like" },
      { label: "PW: Dislike", value: "--action dislike" },
      { label: "PW: Pause", value: "--action pause" },
    ]);
  });
});
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```powershell
npx vitest run src/components/commandLineModel.test.ts
```

Expected: FAIL because `./commandLineModel` does not exist.

- [ ] **Step 3: Implement the immutable command-line model**

Create `src/components/commandLineModel.ts`:

```ts
export const COMMAND_LINE_TITLE = "Command Line";
export const COMMAND_LINE_SUBTITLE = "Local CLI actions";

export const commandLineRows = [
  { label: "PW: Next", value: "--action next" },
  { label: "PW: Like", value: "--action like" },
  { label: "PW: Dislike", value: "--action dislike" },
  { label: "PW: Pause", value: "--action pause" },
] as const;
```

- [ ] **Step 4: Wire the Sidebar and Inspector to the model**

Add to `Sidebar.vue`:

```ts
import { COMMAND_LINE_TITLE } from "./commandLineModel";
```

Replace only the `shortcuts` entry in `systemLinks` with:

```ts
{ key: "shortcuts", label: COMMAND_LINE_TITLE, icon: "shortcuts" },
```

Add to `InspectorPanel.vue`:

```ts
import {
  COMMAND_LINE_SUBTITLE,
  COMMAND_LINE_TITLE,
  commandLineRows,
} from "./commandLineModel";
```

Delete the local `const shortcutRows = [...]` block at current lines 24-29.

Replace the complete `workspaceSection === 'shortcuts'` section with:

```vue
<section v-else-if="store.workspaceSection === 'shortcuts'" class="inspector-panel system-panel">
  <div class="system-panel__header">
    <AppIcon name="shortcuts" />
    <div>
      <div class="system-panel__title">{{ COMMAND_LINE_TITLE }}</div>
      <div class="system-panel__subtitle">{{ COMMAND_LINE_SUBTITLE }}</div>
    </div>
    <IconButton icon="x" label="Close command line" variant="quiet" @click="closePanel" />
  </div>
  <div class="system-list">
    <div v-for="row in commandLineRows" :key="row.label" class="system-list-row">
      <AppIcon name="chevron-right" />
      <span>{{ row.label }}</span>
      <strong>{{ row.value }}</strong>
    </div>
  </div>
</section>
```

- [ ] **Step 5: Verify GREEN and production typing**

Run:

```powershell
npx vitest run src/components/commandLineModel.test.ts
npx vue-tsc --noEmit
```

Expected: 1 focused test passes and `vue-tsc` exits 0.

- [ ] **Step 6: Append Task 2 process evidence**

Append to the active Phase 1 entry in `CHANGELOG_AI.md`:

```markdown
### Task 2 — Accurate command-line terminology

- Replaced the misleading Shortcuts copy with Command Line while preserving the internal workspace key and all existing CLI arguments.
- Added an executable presentation model test instead of a source-string assertion.
- Focused Vitest: PASS, 1/1 after the missing-module RED.
- `npx vue-tsc --noEmit`: PASS.
- No CLI behavior, Tauri command, registry entry, or system setting changed.
```

- [ ] **Step 7: Review and commit only Task 2 code**

Run:

```powershell
git diff -- src/components/Sidebar.vue src/components/InspectorPanel.vue src/components/commandLineModel.ts src/components/commandLineModel.test.ts
git add -- src/components/Sidebar.vue src/components/commandLineModel.ts src/components/commandLineModel.test.ts
git add -p -- src/components/InspectorPanel.vue
git diff --cached --check
git diff --cached
git commit -m "fix(ui): label command-line controls accurately"
```

During `git add -p`, stage only the import/model/copy changes listed above. Reject the pre-existing display-title and metadata hunks. If Git cannot split the relevant hunk safely, leave `InspectorPanel.vue` unstaged and do not create a partial Task 2 commit.

---

### Task 3: Centralize and test playback rating eligibility

**Files:**
- Create: `src/stores/wallpaperCommandRouting.test.ts`
- Modify: `src-tauri/src/db.rs:140-173`
- Modify: `src-tauri/src/db.rs:1543-1564`
- Modify: `src-tauri/src/db.rs` test module after the weighted sampling test
- Modify: `docs/project-docs/CHANGELOG_AI.md`

**Interfaces:**
- Consumes: Pinia's existing `createCollection`, `like`, `dislike`, and `resetRating` actions; Tauri command names `create_collection`, `like_wallpaper`, `dislike_wallpaper`, and `reset_rating`; database ratings `-1`, `0`, and `1`; `Database::get_next_wallpapers(count)`; existing `sample_weighted_without_replacement` call sites.
- Produces: executable Store-to-Tauri routing coverage and private `playback_ticket_weight(rating: i32) -> Option<usize>`; unchanged public database and Tauri interfaces.

- [ ] **Step 1: Add Store-to-Tauri command-routing characterization tests**

Create `src/stores/wallpaperCommandRouting.test.ts`:

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: (path: string) => `asset://${path}`,
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => undefined),
}));

import { useWallpaperStore } from "./wallpapers";

const emptyStats = {
  total: 0,
  liked: 0,
  disliked: 0,
  blacklisted: 0,
  total_plays: 0,
};

describe("wallpaper Store command routing", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "get_stats") return emptyStats;
      if (command === "create_collection") {
        return {
          id: 7,
          name: "Weekend",
          color: "#5b8def",
          wallpaper_count: 0,
        };
      }
      return undefined;
    });
  });

  it("routes collection creation to create_collection", async () => {
    const store = useWallpaperStore();

    await store.createCollection("Weekend", "#5b8def");

    expect(invokeMock).toHaveBeenCalledOnce();
    expect(invokeMock).toHaveBeenCalledWith("create_collection", {
      name: "Weekend",
      color: "#5b8def",
    });
  });

  it("routes Like, Dislike, and restore to their exact rating commands", async () => {
    const store = useWallpaperStore();
    const path = "D:/walls/current.jpg";

    await store.like(path);
    expect(invokeMock).toHaveBeenNthCalledWith(1, "like_wallpaper", { path });
    expect(invokeMock).toHaveBeenNthCalledWith(2, "get_stats");

    invokeMock.mockClear();
    await store.dislike(path);
    expect(invokeMock).toHaveBeenNthCalledWith(1, "dislike_wallpaper", { path });
    expect(invokeMock).toHaveBeenNthCalledWith(2, "get_stats");

    invokeMock.mockClear();
    await store.resetRating(path);
    expect(invokeMock).toHaveBeenNthCalledWith(1, "reset_rating", { path });
    expect(invokeMock).toHaveBeenNthCalledWith(2, "get_stats");
  });
});
```

Run:

```powershell
npx vitest run src/stores/wallpaperCommandRouting.test.ts
```

Expected: PASS, 2/2. This is characterization coverage of existing correct routing; the Rust sampler change below supplies the task's RED/GREEN cycle.

- [ ] **Step 2: Strengthen the sampling test so it fails with the current implementation**

Replace the existing `weighted_sampling_gives_liked_paths_two_tickets_and_never_repeats` test with:

```rust
#[test]
fn playback_sampling_excludes_disliked_and_keeps_liked_two_tickets() {
    let candidates = vec![
        ("disliked".to_string(), -1),
        ("liked".to_string(), 1),
        ("normal-a".to_string(), 0),
        ("normal-b".to_string(), 0),
    ];

    for ticket in [0, 1] {
        let selected = sample_weighted_without_replacement(candidates.clone(), 1, |_| ticket);
        assert_eq!(selected, vec!["liked"]);
    }
    let normal_ticket = sample_weighted_without_replacement(candidates.clone(), 1, |_| 2);
    assert_eq!(normal_ticket, vec!["normal-a"]);

    let selected = sample_weighted_without_replacement(candidates, 3, |_| 0);
    assert_eq!(selected, vec!["liked", "normal-a", "normal-b"]);
    assert_eq!(
        selected.iter().collect::<HashSet<_>>().len(),
        selected.len()
    );
}
```

- [ ] **Step 3: Run the focused Rust test and verify RED**

Run from `src-tauri`:

```powershell
cargo test playback_sampling_excludes_disliked_and_keeps_liked_two_tickets -- --nocapture
```

Expected: one test runs and FAILS because the current sampler treats `rating = -1` as a one-ticket candidate.

- [ ] **Step 4: Implement one private playback weight contract and use it in the sampler**

Insert before `sample_weighted_without_replacement`:

```rust
fn playback_ticket_weight(rating: i32) -> Option<usize> {
    if rating < 0 {
        None
    } else if rating == 1 {
        Some(2)
    } else {
        Some(1)
    }
}
```

Replace the complete sampling helper with:

```rust
fn sample_weighted_without_replacement<F>(
    mut candidates: Vec<(String, i32)>,
    requested: usize,
    mut ticket_for: F,
) -> Vec<String>
where
    F: FnMut(usize) -> usize,
{
    candidates.retain(|(_, rating)| playback_ticket_weight(*rating).is_some());
    let mut selected = Vec::with_capacity(requested.min(candidates.len()));

    while selected.len() < requested && !candidates.is_empty() {
        let total_weight = candidates
            .iter()
            .filter_map(|(_, rating)| playback_ticket_weight(*rating))
            .sum::<usize>();
        let mut ticket = ticket_for(total_weight) % total_weight;
        let selected_index = candidates
            .iter()
            .position(|(_, rating)| {
                let weight = playback_ticket_weight(*rating)
                    .expect("ineligible candidates must be removed before sampling");
                if ticket < weight {
                    true
                } else {
                    ticket -= weight;
                    false
                }
            })
            .expect("weighted ticket must select a candidate");
        let (path, _) = candidates.remove(selected_index);
        selected.push(path);
    }

    selected
}
```

- [ ] **Step 5: Add a database integration regression for all exclusion flags**

Add after the sampling test:

```rust
#[test]
fn next_wallpapers_exclude_negative_hidden_and_unavailable_rows() {
    let db_path = unique_temp_db_path("playback-eligibility");
    remove_sqlite_files(&db_path);
    let db = Database::new(&db_path).expect("database should initialize");
    let files = ["normal", "liked", "disliked", "hidden", "unavailable"]
        .map(|name| db_path.with_file_name(format!("{name}.jpg")));

    for (index, path) in files.iter().enumerate() {
        std::fs::write(path, b"image").expect("wallpaper file should exist");
        db.upsert_wallpaper(
            &path.to_string_lossy(),
            &format!("hash-{index}"),
            "test",
            100,
            100,
            10,
        )
        .expect("wallpaper should insert");
    }

    let normal = files[0].to_string_lossy().to_string();
    let liked = files[1].to_string_lossy().to_string();
    let disliked = files[2].to_string_lossy().to_string();
    let hidden = files[3].to_string_lossy().to_string();
    let unavailable = files[4].to_string_lossy().to_string();
    db.set_rating(&liked, 1).expect("liked rating should save");
    db.set_rating(&disliked, -1)
        .expect("disliked rating should save");
    db.set_blacklisted(&hidden, true)
        .expect("hidden state should save");
    db.mark_paths_unavailable(std::slice::from_ref(&unavailable))
        .expect("unavailable state should save");

    let selected = db
        .get_next_wallpapers(8)
        .expect("eligible wallpapers should be selected");
    let eligible = HashSet::from([normal, liked]);
    assert_eq!(selected.len(), 8);
    assert!(selected.iter().all(|path| eligible.contains(path)));
    assert_eq!(selected.into_iter().collect::<HashSet<_>>(), eligible);

    drop(db);
    for path in files {
        let _ = std::fs::remove_file(path);
    }
    remove_sqlite_files(&db_path);
}
```

- [ ] **Step 6: Verify the playback contract GREEN**

Run from `src-tauri`:

```powershell
cargo test playback_sampling_excludes_disliked_and_keeps_liked_two_tickets -- --nocapture
cargo test next_wallpapers_exclude_negative_hidden_and_unavailable_rows -- --nocapture
cargo test db::tests -- --nocapture
cargo fmt -- --check
cd ..
npx vitest run src/stores/wallpaperCommandRouting.test.ts
```

Expected: each focused Rust command runs one matching test and passes; all database tests and both command-routing tests pass; formatting passes.

- [ ] **Step 7: Append Task 3 process evidence**

Append to the active Phase 1 entry in `CHANGELOG_AI.md`:

```markdown
### Task 3 — Playback rating eligibility contract

- Centralized the private playback ticket rule: negative ratings are excluded, liked items own two tickets, and other non-negative items own one.
- Added a database integration regression proving disliked, hidden, and unavailable rows never enter playback candidates.
- Added Pinia command-routing coverage for collection creation plus Like, Dislike, and rating reset/restore.
- Focused Rust RED reproduced direct sampling of a disliked candidate; both focused tests and all database tests pass after the repair.
- Focused Store command routing: PASS, 2/2.
- `cargo fmt -- --check`: PASS.
- Public database methods, Tauri commands, SQLite schema, registry state, and system settings are unchanged.
```

- [ ] **Step 8: Review and commit Task 3**

Run:

```powershell
git diff -- src-tauri/src/db.rs src/stores/wallpaperCommandRouting.test.ts
git diff --check -- src-tauri/src/db.rs src/stores/wallpaperCommandRouting.test.ts
git add -- src-tauri/src/db.rs src/stores/wallpaperCommandRouting.test.ts
git diff --cached --check
git commit -m "fix(playback): centralize rating eligibility"
```

Expected: the commit contains only the private playback rule and its Rust/Pinia regressions.

---

### Task 4: Keep System navigation reachable at 800x600

**Files:**
- Modify: `src/styles.css:684-693`
- Modify: `src/styles.css:906-911`
- Modify: `docs/project-docs/CHANGELOG_AI.md`

**Interfaces:**
- Consumes: existing `.side-rail` flex layout, nested tag-list scrolling, configured 800x600 Tauri minimum window.
- Produces: an independently scrollable sidebar whose System group retains intrinsic height and becomes reachable by wheel, touchpad, or keyboard focus scrolling.

- [ ] **Step 1: Capture the current failing minimum-window state**

Run:

```powershell
npm run build
```

Then use the `browser-qa` workflow with a read-only Tauri IPC mock, as documented by `AI_DIARY.md #frontend-qa-001` and `#frontend-qa-002`, to render the production bundle at exactly 800x600. Save the evidence outside the repository as:

```text
C:\tmp\purewall-phase1-sidebar-red-800x600.png
```

Expected RED: the bottom System entries, including Advanced, are not all reachable in the visible sidebar.

- [ ] **Step 2: Make the side rail own vertical overflow**

Add these declarations to the existing `.side-rail` block:

```css
  overflow-y: auto;
  overscroll-behavior: contain;
  scrollbar-gutter: stable;
```

The complete affected block becomes:

```css
.side-rail {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  overflow-y: auto;
  overscroll-behavior: contain;
  scrollbar-gutter: stable;
  padding: var(--space-5) var(--space-3) var(--space-3);
  border-right: 1px solid var(--border-subtle);
  background: var(--bg-sidebar);
}
```

Add one flex constraint to `.sidebar-system`:

```css
.sidebar-system {
  flex: 0 0 auto;
  display: grid;
  gap: var(--space-2);
  padding-top: var(--space-4);
  border-top: 1px solid var(--border-subtle);
}
```

Do not alter the existing 920px or 700px responsive information architecture in this task.

- [ ] **Step 3: Verify minimum-window reachability GREEN**

Run:

```powershell
npx vue-tsc --noEmit
npm run build
```

Repeat the read-only production-bundle QA at 800x600 and save:

```text
C:\tmp\purewall-phase1-sidebar-green-800x600.png
```

Verify all of the following:

1. Mouse wheel/touchpad scrolling reaches Displays, Settings, Insights, Command Line, and Advanced.
2. Repeated Tab navigation scrolls each focused System item into view.
3. The main gallery does not move when the pointer scrolls inside the sidebar.
4. The nested Tags and Collections lists still scroll independently when they overflow.
5. At 1200x800, the sidebar retains the current visual spacing and no unnecessary horizontal scrollbar appears.

- [ ] **Step 4: Append Task 4 process evidence**

Append to the active Phase 1 entry in `CHANGELOG_AI.md`:

```markdown
### Task 4 — Minimum-window System navigation

- The side rail now owns vertical overflow and keeps the System section at intrinsic height, making every navigation item reachable at 800x600.
- Read-only production-bundle QA captured the before/after 800x600 states outside the repository.
- `npx vue-tsc --noEmit`: PASS.
- `npm run build`: PASS.
- No Tauri window configuration, registry entry, system setting, or application data was changed.
```

- [ ] **Step 5: Review and commit only the Phase 1 CSS hunk**

Run:

```powershell
git diff -- src/styles.css
git add -p -- src/styles.css
git diff --cached --check
git diff --cached
git commit -m "fix(layout): keep sidebar navigation reachable"
```

Stage only the `.side-rail` overflow declarations and `.sidebar-system` flex declaration. Reject all pre-existing style changes. If the hunk cannot be isolated safely, leave the CSS unstaged and do not create a partial Task 4 commit.

---

### Task 5: Run the Phase 1 completion gate and close process documentation

**Files:**
- Modify: `docs/project-docs/CHANGELOG_AI.md`
- Modify only if a new pitfall occurred: `docs/project-docs/AI_DIARY.md`
- Do not modify: `docs/project-docs/ARCHITECTURE.md`
- Do not modify: `docs/project-docs/DECISIONS.md`

**Interfaces:**
- Consumes: all Task 1-4 changes and their focused verification evidence.
- Produces: a verified Phase 1 working tree and a complete process-documentation record; no new application interface.

- [ ] **Step 1: Run all focused frontend and backend regressions together**

Run from repository root:

```powershell
npx vitest run src/components/sidebarModel.test.ts src/components/commandLineModel.test.ts src/stores/wallpaperCommandRouting.test.ts
cd src-tauri
cargo test playback_sampling_excludes_disliked_and_keeps_liked_two_tickets -- --nocapture
cargo test next_wallpapers_exclude_negative_hidden_and_unavailable_rows -- --nocapture
cd ..
```

Expected: frontend focused tests pass 5/5; each Rust command executes and passes one matching test.

- [ ] **Step 2: Run the full completion gate**

Run:

```powershell
cd src-tauri
cargo fmt -- --check
cargo check
cargo clippy --all-targets -- -D warnings
cargo test
cd ..
npx vue-tsc --noEmit
npm run test:unit
npm run build
git diff --check
```

Expected baseline after this plan: Rust reports 59 passed and 1 ignored manual performance benchmark; Vitest reports 18 passed across 8 files; every command exits 0. If counts differ because another verified user change added tests, record the actual greater count and ensure no test disappeared.

- [ ] **Step 3: Audit scope and safety evidence**

Run:

```powershell
git status --short
git diff -- src/components/Sidebar.vue src/components/InspectorPanel.vue src/components/sidebarModel.ts src/components/sidebarModel.test.ts src/components/commandLineModel.ts src/components/commandLineModel.test.ts src-tauri/src/db.rs src/styles.css docs/project-docs/CHANGELOG_AI.md
Select-String -Path src-tauri\src\context_menu.rs,src-tauri\src\autostart.rs -Pattern 'HKLM|Policies|86ca1aa0' -CaseSensitive:$false
```

Expected: Phase 1 touched no registry/autostart implementation; existing safety constraints remain unchanged. The search is read-only and must not be followed by any registry command.

- [ ] **Step 4: Close the Phase 1 changelog entry**

Append:

```markdown
### Phase 1 final verification

- Corrected collection creation, added Disliked navigation, accurately labeled CLI actions, centralized playback rating eligibility, and made System navigation reachable at 800x600.
- Focused frontend regressions: PASS, 5/5.
- Focused Rust playback regressions: PASS, 2/2.
- `cargo fmt -- --check`: PASS.
- `cargo check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `cargo test`: PASS, 59 passed and 1 ignored manual performance benchmark.
- `npx vue-tsc --noEmit`: PASS.
- `npm run test:unit`: PASS, 18 passed across 8 files.
- `npm run build`: PASS; only previously documented upstream warnings may remain.
- `git diff --check`: PASS.
- No registry command was executed, no PureWall-owned registry entry changed, and no system setting changed.

### Unresolved items

- Global keyboard shortcuts remain outside Phase 1; the existing CLI actions are now labeled Command Line.
- Phase 2 action-path consolidation starts only after explicit user approval of this completed milestone.
```

If a genuinely new pitfall occurred, append one new entry to `AI_DIARY.md` with phenomenon, cause, fix, and lesson. If no new pitfall occurred, leave `AI_DIARY.md` unchanged. Do not update `ARCHITECTURE.md` or `DECISIONS.md` because this plan changes neither system boundaries nor accepted architecture.

- [ ] **Step 5: Perform the final staging audit**

Run:

```powershell
git diff --check
git status --short
git diff --cached --check
git diff --cached
```

Do not stage the entire dirty `CHANGELOG_AI.md`. Leave its Phase 1 record in the working tree unless it can be isolated without including pre-existing user changes. Report that preservation choice in the handoff. Do not claim Phase 1 complete unless every required command and visual check passed.
