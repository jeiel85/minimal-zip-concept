# GitHub Release & Auto-update Testing Guide

This guide explains how to publish releases on GitHub and how to test the automatic update feature of MZC.

---

## 1. Creating an Official GitHub Release

To trigger the MZC auto-updater for end users, you need to create a release on GitHub.

1. Navigate to your repository page: `https://github.com/jeiel85/minimal-zip-concept`.
2. On the right sidebar, click on **Releases** -> **Create a new release** (or **Draft a new release**).
3. Set the **Tag version** to the Cargo package version with a leading `v` (for example, `v0.12.0`).
4. Set the **Release title** to the same version (for example, `MZC v0.12.0`).
5. In the description, write user-facing release notes based on `CHANGELOG.md`.
6. Before publishing, run the release verification sequence in `docs/TEST_PLAN.md`.
7. **Important**: Under **Attach binaries by dropping them here**, upload the installer package:
   - `mzc-setup.exe` (compiled using Inno Setup).
8. Click **Publish release**.

### Crates.io publishing

The release workflow attempts `cargo publish` only when the repository secret
`CARGO_REGISTRY_TOKEN` is configured. If the secret is empty or missing, the
Crates.io job records the skip and finishes successfully. This keeps GitHub
Releases independent from Crates.io availability while preserving an automatic
publish path for maintainers who opt in by adding the token.

---

## 2. Testing the Auto-update Prompt Locally

Since the updater compares the GitHub Release tag name against the compiled version of the app (`env!("CARGO_PKG_VERSION")`), you can simulate an update without publishing a real public release.

### Method A: Prerelease / Draft Test on GitHub
1. Create a release on GitHub with a higher tag name than the current Cargo package version (for example, if Cargo is `0.12.0`, tag it `v0.12.1-test`).
2. Attach `mzc-setup.exe` as an asset.
3. Run the local MZC desktop GUI.
4. Click **Check for updates**.
5. The GUI will detect that the test tag is newer, display the release notes from the release description, and let you click "Update Now" to download and execute the setup automatically.

### Method B: Local Code Mocking (No GitHub Upload Required)
To verify the GUI modal design, progress bar, and process replacement flow locally without uploading to GitHub:
1. Open `src/gui.rs`.
2. Locate `spawn_check_update_task()`.
3. Temporarily bypass the version comparison logic to force-trigger the update prompt:
   ```rust
   // Find this line:
   if is_newer_version(&info.version, current_version) {
       let _ = tx.send(TaskResult::UpdateCheckResult(Ok(info)));
   }
   
   // Replace it temporarily with:
   if true {
       let mut mock_info = info.clone();
       mock_info.version = "0.12.1-mocked".to_string();
       let _ = tx.send(TaskResult::UpdateCheckResult(Ok(mock_info)));
   }
   ```
4. Run `cargo run -- gui`.
5. Click **Check for updates**. It will immediately pop up the update modal and allow you to test downloading the latest executable from GitHub.
