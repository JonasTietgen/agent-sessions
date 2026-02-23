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

/// Returns the canonical editor name if the process is a VS Code-compatible editor,
/// None otherwise.
fn classify_vscode_process(name: &str) -> Option<String> {
    // Match only the main process names, not helpers/renderers.
    // On macOS, VS Code and its forks run as "Code", "Cursor", "Windsurf", etc.
    // "Code Helper (Renderer)" and similar are NOT the main window process.
    let trimmed = name.trim();
    match trimmed {
        "Code" => Some("Code".to_string()),
        "Cursor" => Some("Cursor".to_string()),
        "Windsurf" => Some("Windsurf".to_string()),
        _ => {
            // "Code - Insiders" appears as a single comm string on some systems
            if trimmed.starts_with("Code ") && !trimmed.contains("Helper") {
                Some(trimmed.to_string())
            } else {
                None
            }
        }
    }
}

/// Focus the VS Code (or compatible editor) window that contains this session.
/// Tries each known editor process name in order.
pub fn focus_vscode() -> Result<(), String> {
    let script = r#"
        tell application "System Events"
            if exists process "Code" then
                set frontmost of process "Code" to true
                return "found"
            end if
            if exists process "Cursor" then
                set frontmost of process "Cursor" to true
                return "found"
            end if
            if exists process "Windsurf" then
                set frontmost of process "Windsurf" to true
                return "found"
            end if
        end tell
        return "not found"
    "#;
    execute_applescript(script)
}

/// Focus a specific VS Code-compatible editor by its process name
pub fn focus_vscode_by_name(editor_name: &str) -> Result<(), String> {
    let script = format!(
        r#"
        tell application "System Events"
            if exists process "{}" then
                set frontmost of process "{}" to true
                return "found"
            end if
        end tell
        return "not found"
    "#,
        editor_name, editor_name
    );
    execute_applescript(&script)
}
