# GitHub Release & Auto-update Testing Guide

This guide explains how to publish releases on GitHub and how to test the automatic update feature of MZC.

---

## 1. Creating a Official GitHub Release

To trigger the MZC auto-updater for end users, you need to create a release on GitHub.

1. Navigate to your repository page: `https://github.com/jeiel85/minimal-zip-concept`.
2. On the right sidebar, click on **Releases** -> **Create a new release** (or **Draft a new release**).
3. Set the **Tag version** (e.g., `v0.10.0` or `v1.0.0`).
4. Set the **Release title** (e.g., `MZC v0.10.0`).
5. In the description, write the release notes (you can copy the content inside the `<ko-KR>` or `<en-US>` tags in `C:\Users\jeiel\Desktop\Build\release_notes_v0.9.0.txt`).
6. **Important**: Under **Attach binaries by dropping them here**, upload the installer package:
   - `mzc-setup.exe` (compiled using Inno Setup).
7. Click **Publish release**.

---

## 2. Testing the Auto-update Prompt Locally

Since the updater compares the GitHub Release tag name against the compiled version of the app (`env!("CARGO_PKG_VERSION")`), you can simulate an update without publishing a real public release.

### Method A: Prerelease / Draft Test on GitHub
1. Create a release on GitHub with a higher tag name than the current cargo package version (e.g., if cargo is `0.9.0`, tag it `v0.9.9-test`).
2. Attach `mzc-setup.exe` as an asset.
3. Run the local MZC desktop GUI (running `0.9.0`).
4. Click **🌐 최신 업데이트 확인** (Check for updates).
5. The GUI will detect that `0.9.9-test` is newer, display the release notes (from the release description), and let you click "Update Now" to download and execute the setup automatically.

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
       mock_info.version = "0.9.9-mocked".to_string();
       let _ = tx.send(TaskResult::UpdateCheckResult(Ok(mock_info)));
   }
   ```
4. Run `cargo run -- gui`.
5. Click **🌐 최신 업데이트 확인**. It will immediately pop up the update modal and allow you to test downloading the latest executable from GitHub.
