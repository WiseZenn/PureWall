# PureWall Phase 3B Batch Management Implementation Plan

> **For Codex:** REQUIRED SUB-SKILL: Use `executing-plans` to implement this plan task by task, and use `test-driven-development` for every behavior change.

**Goal:** Complete PureWall's existing multi-select workflow with safe, transactional batch rating, hide/restore, tag, and collection operations while preserving the current Recycle Bin delete and delayed Undo behavior.

**Architecture:** Keep Tauri commands narrow and add a small batch-boundary module for input normalization, a bounded maximum, and the serialized mutation result. Database mutations remain in `db.rs`, each inside one SQLite transaction and returning the actual affected-row count. The Pinia store owns success/failure selection semantics and consumes backend refresh hints; `Gallery.vue` remains presentation-only and uses a small pure model for relation-menu actions.

**Tech Stack:** Tauri 2, Rust, rusqlite, Vue 3, TypeScript, Pinia, Vitest, CSS.

**Accepted boundaries:** ADR-007 and ADR-021 already cover normalized tags, blacklist state, and the separation of tags from collections. Phase 3B introduces no new architectural decision, so no new ADR is required.

---

## Task 1: Define and verify the bounded batch command contract

**Files:**
- Create: `src-tauri/src/batch_operations.rs`
- Modify: `src-tauri/src/main.rs`

**Step 1: Write failing boundary tests**

Add unit tests for a wished-for `normalize_batch_paths` API:

- preserves the first occurrence order while removing duplicate paths;
- rejects an empty batch;
- rejects more than `MAX_BATCH_PATHS` raw inputs;
- uses a fixed `MAX_BATCH_PATHS` value of 500.

Add the module declaration required for the tests to compile, with the production function initially stubbed.

**Step 2: Run the focused test and verify RED**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml batch_operations
```

Expected: FAIL because normalization is not implemented.

**Step 3: Implement the minimal boundary module**

Implement:

- `MAX_BATCH_PATHS: usize = 500`;
- `normalize_batch_paths(&[String]) -> CommandResult<Vec<String>>`;
- `BatchRefreshTarget` serialized as snake_case strings;
- `BatchMutationResult { affected, refresh }`.

Reject oversized input before path validation, trim each path, reject empty values, and deduplicate without reordering.

**Step 4: Verify GREEN**

Run the focused Rust test again and confirm all boundary tests pass.

**Step 5: Commit**

```powershell
git add src-tauri/src/batch_operations.rs src-tauri/src/main.rs
git -c commit.gpgsign=false commit -m "feat(phase3): define bounded batch contract"
```

## Task 2: Make metadata mutations transactional, validated, and countable

**Files:**
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/main.rs`

**Step 1: Write failing database tests**

Add tests that prove:

- rating and blacklist batches return affected counts;
- tag assignment rolls back the entire transaction when a test trigger aborts the second insert;
- batch tag removal deletes existing links and is idempotent;
- batch collection removal deletes existing links and is idempotent.

The rollback test must verify that the first successful insert is absent after the second insert aborts.

**Step 2: Run the focused tests and verify RED**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml batch_
```

Expected: FAIL because count-returning methods and batch unassign methods do not yet exist.

**Step 3: Implement minimal database methods**

Update the four existing batch methods to return `Result<usize>`, summing affected rows inside the existing transaction. Add:

- `batch_unassign_tag`;
- `batch_unassign_collection`;
- `tag_exists`;
- `collection_exists`.

Do not mutate source image files.

**Step 4: Wire and validate Tauri commands**

In `main.rs`:

- normalize and bound every metadata batch before validating registered files;
- accept ratings only in `-1`, `0`, or `1`;
- require positive tag/collection IDs and verify the related entity exists;
- add `batch_unassign_tag` and `batch_unassign_collection`;
- return `BatchMutationResult` with the necessary refresh targets;
- register both new commands in the Tauri invoke handler.

Keep `batch_delete_wallpapers` and its frontend delayed-commit Undo flow unchanged.

**Step 5: Verify GREEN and the Rust gate**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml batch_
cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: PASS with zero errors.

**Step 6: Commit**

```powershell
git add src-tauri/src/db.rs src-tauri/src/main.rs
git -c commit.gpgsign=false commit -m "feat(phase3): complete transactional batch commands"
```

## Task 3: Enforce success and failure selection semantics in Pinia

**Files:**
- Create: `src/stores/batchOperations.test.ts`
- Modify: `src/stores/wallpapers.ts`

**Step 1: Write failing store tests**

Using the existing Tauri invoke mocks, test:

- a successful mutation returns `true`, clears selection, and follows the backend refresh hints;
- a rejected mutation returns `false` and preserves selection;
- tag and collection removal route to the exact new command names and arguments;
- repeated clicks are blocked while a batch mutation is already running.

Do not test or change the delayed Recycle Bin deletion flow in this task.

**Step 2: Run the focused test and verify RED**

Run:

```powershell
npm run test:unit -- src/stores/batchOperations.test.ts
```

Expected: FAIL because the shared batch runner, busy state, boolean result, and removal methods are missing.

**Step 3: Implement the minimal store behavior**

Add:

- a typed `BatchMutationResult`;
- `isBatchMutating`;
- one shared mutation runner that snapshots selected paths, consumes refresh hints, clears selection only after command success, and preserves it on failure;
- `batchUnassignTag` and `batchUnassignCollection`;
- boolean return values for metadata batch actions.

Retain the current optimistic rating/blacklist page updates and the hidden-item Undo notification. Do not route `batchDelete` through the new runner.

**Step 4: Verify GREEN**

Run the focused Vitest file, then `npx vue-tsc --noEmit`.

**Step 5: Commit**

```powershell
git add src/stores/batchOperations.test.ts src/stores/wallpapers.ts
git -c commit.gpgsign=false commit -m "feat(phase3): preserve batch selection on failure"
```

## Task 4: Complete the compact batch toolbar

**Files:**
- Create: `src/components/batchRelationMenuModel.ts`
- Create: `src/components/batchRelationMenuModel.test.ts`
- Modify: `src/components/CompactDropdown.vue`
- Modify: `src/components/Gallery.vue`
- Modify: `src/styles.css`

**Step 1: Write failing menu-model tests**

Test that the pure menu model:

- produces distinct Add and Remove actions for every tag or collection;
- parses a selected action into `{ operation, id }`;
- rejects placeholders and malformed values.

**Step 2: Run the focused test and verify RED**

Run:

```powershell
npm run test:unit -- src/components/batchRelationMenuModel.test.ts
```

Expected: FAIL because the model does not exist.

**Step 3: Implement the model and toolbar**

Use the established `CompactDropdown` interaction pattern, extending its options with optional group labels so Add and Remove remain visually distinct without adding permanent toolbar width. In `Gallery.vue`:

- add Clear rating;
- replace the two add-only native selects with compact Tags and Collections menus;
- route Add/Remove to the matching Pinia methods;
- reset the selected menu value only after a successful mutation;
- disable batch actions while `isBatchMutating`;
- retain create-collection, Hide/Restore, and Delete/Confirm behavior.

Use semantic buttons, dynamic ARIA state, visible focus treatment, the existing icon set, and theme tokens. Do not add animation that shifts layout.

**Step 4: Verify GREEN and UI type safety**

Run:

```powershell
npm run test:unit -- src/components/batchRelationMenuModel.test.ts
npx vue-tsc --noEmit
npm run build
```

Expected: PASS.

**Step 5: Commit**

```powershell
git add src/components/batchRelationMenuModel.ts src/components/batchRelationMenuModel.test.ts src/components/CompactDropdown.vue src/components/Gallery.vue src/styles.css
git -c commit.gpgsign=false commit -m "feat(phase3): complete batch management toolbar"
```

## Task 5: Document, verify, and close Phase 3B

**Files:**
- Modify: `docs/project-docs/CHANGELOG_AI.md`
- Modify: `docs/project-docs/AI_DIARY.md` only if a genuinely new pitfall was encountered

**Step 1: Run the full verification gate**

Run:

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
cargo check --manifest-path src-tauri/Cargo.toml
npm run test:unit
npx vue-tsc --noEmit
npm run build
```

**Step 2: Update process documentation**

Append a Phase 3B entry to `CHANGELOG_AI.md` with the exact code scope, command contract, UI behavior, and verification evidence. Append to `AI_DIARY.md` only for a new reusable pitfall. Record any unresolved item explicitly.

**Step 3: Verify the documentation diff and clean tree**

Run `git diff --check`, review `git diff --stat`, then commit the documentation.

```powershell
git add docs/project-docs/CHANGELOG_AI.md docs/project-docs/AI_DIARY.md docs/superpowers/plans/2026-07-28-purewall-phase-3b-batch-management.md
git -c commit.gpgsign=false commit -m "docs(phase3): record batch management completion"
```

**Step 4: Apply the branch completion workflow**

Use `verification-before-completion`, then `finishing-a-development-branch`. Do not merge or push without the user's explicit choice.
