use super::applescript::execute_applescript;
use std::process::Command;

/// Check if the process with the given PID is running inside VS Code's integrated terminal
/// by walking up the process parent chain looking for a VS Code (or compatible editor) process
pub fn is_vscode_terminal(pid: u32) -> bool {
    detect_vscode_ancestor(pid).is_some()
}

/// Detect which VS Code-compatible editor is an ancestor of the given PID.
/// Returns the editor process name if found, None otherwise.
pub fn detect_vscode_ancestor(pid: u32) -> Option<String> {
    let mut current_pid = pid;
    for _ in 0..10 {
        let ppid = match get_parent_pid(current_pid) {
            Some(p) if p > 1 => p,
            _ => return None,
        };
        if let Some(name) = get_process_name(ppid) {
            if let Some(editor) = classify_vscode_process(&name) {
                return Some(editor);
            }
        }
        current_pid = ppid;
    }
    None
}

fn get_parent_pid(pid: u32) -> Option<u32> {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "ppid="])
        .output()
        .ok()?;
    if output.status.success() {
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<u32>()
            .ok()
    } else {
        None
    }
}

fn get_process_name(pid: u32) -> Option<String> {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

/// Returns the System Events process name if this process belongs to a VS Code-compatible
/// editor, None otherwise.
///
/// On macOS, `ps -o comm=` returns the full executable path, e.g.:
///   /Applications/Visual Studio Code.app/Contents/Frameworks/Code Helper.app/Contents/MacOS/Code Helper
///   /Applications/Visual Studio Code.app/Contents/MacOS/Electron
///
/// The System Events process name (used in AppleScript `exists process "…"`) is the
/// CFBundleName of the host app bundle: VS Code → "Code", Cursor → "Cursor", etc.
fn classify_vscode_process(name: &str) -> Option<String> {
    let trimmed = name.trim();

    // Fast path: some systems return only the binary name
    match trimmed {
        "Code" => return Some("Code".to_string()),
        "Cursor" => return Some("Cursor".to_string()),
        "Windsurf" => return Some("Windsurf".to_string()),
        _ => {}
    }

    // Full-path matching — check for known .app bundle names inside the path.
    // VS Code Insiders must be checked before plain VS Code to avoid a false match.
    if trimmed.contains("Visual Studio Code - Insiders.app") {
        return Some("Code - Insiders".to_string());
    }
    if trimmed.contains("Visual Studio Code.app") {
        return Some("Code".to_string());
    }
    if trimmed.contains("/Cursor.app/") {
        return Some("Cursor".to_string());
    }
    if trimmed.contains("/Windsurf.app/") {
        return Some("Windsurf".to_string());
    }

    None
}

/// Focus the VS Code window for the given project.
///
/// Strategy:
/// 1. Find the VS Code window whose title contains the project name and raise it.
///    VS Code window titles are like "project-name — Visual Studio Code".
/// 2. If no matching window is found, just bring VS Code to the foreground.
///
/// Note: VS Code does not expose terminal tabs via AppleScript, so we can only
/// navigate to the correct *window*. The terminal that was last active in that
/// window will remain visible — which is the expected behaviour for single-window
/// single-project setups.
pub fn focus_vscode_window(editor_name: &str, project_path: &str) -> Result<(), String> {
    // Extract the project folder name from the path (last non-empty component).
    let project_name = project_path
        .split('/')
        .filter(|s| !s.is_empty())
        .last()
        .unwrap_or(project_path);

    // Escape any double-quotes in the project name before embedding in AppleScript.
    let safe_project = project_name.replace('"', "\\\"");
    let safe_editor  = editor_name.replace('"', "\\\"");

    let script = format!(
        r#"
        tell application "System Events"
            if not (exists process "{editor}") then
                return "not found"
            end if
            tell process "{editor}"
                -- Try to raise the window whose title contains the project name.
                set matchedWindow to missing value
                repeat with w in (every window)
                    if name of w contains "{project}" then
                        set matchedWindow to w
                        exit repeat
                    end if
                end repeat
                if matchedWindow is not missing value then
                    set frontmost of process "{editor}" to true
                    perform action "AXRaise" of matchedWindow
                    return "found"
                else
                    -- No window matched: just bring the editor to the front.
                    set frontmost of process "{editor}" to true
                    return "focused"
                end if
            end tell
        end tell
        return "not found"
        "#,
        editor = safe_editor,
        project = safe_project,
    );

    execute_applescript(&script)
}
