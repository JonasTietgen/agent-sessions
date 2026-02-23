use super::applescript::execute_applescript;
use std::io::Write as IoWrite;
use std::net::TcpStream;
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

/// Send a focus request to the VS Code Extension via HTTP POST to localhost:7331.
/// Returns Ok(()) if the extension acknowledged the request (HTTP 200).
fn try_extension_focus(pid: u32) -> Result<(), String> {
    let body = format!("{{\"pid\":{}}}", pid);
    let content_length = body.len();
    let request = format!(
        "POST /focus HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        content_length, body
    );

    let mut stream = TcpStream::connect("127.0.0.1:7331")
        .map_err(|e| format!("Extension not available: {}", e))?;

    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("Failed to send request: {}", e))?;

    // Read just enough to check the status line
    let mut response = [0u8; 15];
    use std::io::Read;
    stream
        .read(&mut response)
        .map_err(|e| format!("Failed to read response: {}", e))?;

    let status_line = String::from_utf8_lossy(&response);
    if status_line.starts_with("HTTP/1.") && status_line.contains("200") {
        Ok(())
    } else {
        Err(format!("Extension returned non-200: {}", status_line.trim()))
    }
}

/// Focus the VS Code window for the given project.
///
/// Strategy:
/// 1. Try the Agent Sessions VS Code Extension (port 7331) for precise terminal-tab navigation.
/// 2. Regardless, bring the correct editor window to the foreground via AppleScript.
/// 3. If the extension is not running, fall back to AppleScript-only (previous behaviour).
pub fn focus_vscode_window(pid: u32, editor_name: &str, project_path: &str) -> Result<(), String> {
    if try_extension_focus(pid).is_ok() {
        // Extension found the terminal — still raise the window so it appears in front.
        return focus_window_applescript(editor_name, project_path);
    }
    // Extension not available: fall back to window-only focus (previous behaviour).
    focus_window_applescript(editor_name, project_path)
}

fn focus_window_applescript(editor_name: &str, project_path: &str) -> Result<(), String> {
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
