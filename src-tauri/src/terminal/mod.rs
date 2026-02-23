mod applescript;
mod iterm;
mod terminal_app;
mod tmux;
pub mod vscode;

use applescript::execute_applescript;

pub use vscode::{detect_vscode_ancestor, is_vscode_terminal};

/// Focus the terminal containing the Claude process with the given PID.
/// `project_path` is used to identify the correct editor window when VS Code is detected.
pub fn focus_terminal_for_pid(pid: u32, project_path: &str) -> Result<(), String> {
    // Check VS Code FIRST, before TTY lookup.
    // VS Code terminals have a TTY, but iTerm2/Terminal.app won't recognise them,
    // so we handle them separately by walking the parent-process chain.
    if let Some(editor_name) = vscode::detect_vscode_ancestor(pid) {
        if vscode::focus_vscode_window(pid, &editor_name, project_path).is_ok() {
            return Ok(());
        }
    }

    // Get the TTY for this process
    let tty = get_tty_for_pid(pid)?;

    // Try tmux next (if the process is running inside tmux)
    if tmux::focus_tmux_pane_by_tty(&tty).is_ok() {
        return Ok(());
    }

    // Try iTerm2
    if iterm::focus_iterm_by_tty(&tty).is_ok() {
        return Ok(());
    }

    // Fall back to Terminal.app
    terminal_app::focus_terminal_app_by_tty(&tty)
}

/// Fallback: focus terminal by matching path in session name
pub fn focus_terminal_by_path(path: &str) -> Result<(), String> {
    // Fallback: focus by matching session name (which often contains the path) in iTerm2
    let script = format!(r#"
        tell application "System Events"
            if exists process "iTerm2" then
                tell application "iTerm2"
                    activate
                    repeat with w in windows
                        repeat with t in tabs of w
                            repeat with s in sessions of t
                                if name of s contains "{}" then
                                    select s
                                    select t
                                    set index of w to 1
                                    return "found"
                                end if
                            end repeat
                        end repeat
                    end repeat
                end tell
            end if
        end tell
        return "not found"
    "#, path.split('/').last().unwrap_or(path));

    execute_applescript(&script)
}

/// Get the TTY device for a given PID using ps command
fn get_tty_for_pid(pid: u32) -> Result<String, String> {
    use std::process::Command;

    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "tty="])
        .output()
        .map_err(|e| format!("Failed to get TTY: {}", e))?;

    if output.status.success() {
        let tty = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if tty.is_empty() || tty == "??" {
            Err("Process has no TTY".to_string())
        } else {
            Ok(tty)
        }
    } else {
        Err("Failed to get TTY for process".to_string())
    }
}
