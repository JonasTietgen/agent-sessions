# Aufgabe: VS Code Extension für Agent Sessions

## Kontext

`Agent Sessions` ist eine macOS Desktop-App (Tauri/Rust + React), die laufende
Claude Code Sessions überwacht und per Klick in den Vordergrund holt.

**Fork:** https://github.com/JonasTietgen/agent-sessions
**Lokal:** `~/Library/CloudStorage/Dropbox/Dokumente/Claude/Tools/agent-sessions/`

VS Code wird bereits korrekt erkannt und das richtige Fenster fokussiert.
**Was noch fehlt:** Navigation zum richtigen Terminal-Tab innerhalb des Fensters.

---

## Was zu bauen ist

### 1. VS Code Extension (`agent-sessions-focus`)

Ordner: `~/Library/CloudStorage/Dropbox/Dokumente/Claude/Tools/agent-sessions-vscode/`

Die Extension startet beim Öffnen von VS Code einen lokalen HTTP-Server auf
Port **7331** und empfängt Focus-Requests von der Agent Sessions App.

**Endpunkt:** `POST http://localhost:7331/focus`
**Body:** `{ "pid": 12345 }` (PID des Claude-Prozesses)

**Logik:**
1. Parent-PID des Claude-Prozesses ermitteln: `ps -p <pid> -o ppid=`
2. Alle offenen Terminals durchsuchen: `vscode.window.terminals`
3. Terminal finden, dessen Shell-PID == Parent-PID von Claude
4. `terminal.show()` aufrufen

**Dateien:**
- `package.json` (Extension Manifest, activationEvent: `onStartupFinished`)
- `src/extension.ts` (HTTP-Server + Terminal-Matching-Logik)
- `tsconfig.json`

**Build:** `npm install && npx vsce package` → erzeugt `agent-sessions-focus-*.vsix`

**Installation für den User:**
```bash
code --install-extension agent-sessions-focus-*.vsix
```

---

### 2. Rust-Änderung in Agent Sessions

**Datei:** `src-tauri/src/terminal/vscode.rs`

In `focus_vscode_window()` vor dem AppleScript-Aufruf einen HTTP-Request
an die Extension schicken:

```rust
// Zuerst Extension versuchen (präzise Tab-Navigation)
if try_extension_focus(pid).is_ok() {
    // Fenster trotzdem nach vorne bringen
    focus_window_applescript(editor_name, project_path)?;
    return Ok(());
}
// Fallback: nur Fenster fokussieren (bisheriges Verhalten)
focus_window_applescript(editor_name, project_path)
```

`try_extension_focus(pid)` schickt einfach einen HTTP-POST zu `localhost:7331/focus`.
Kein extra Crate nötig — `std::net::TcpStream` reicht für einen simplen HTTP-POST.

---

## Reihenfolge

1. VS Code Extension bauen + als `.vsix` paketieren
2. Extension lokal installieren und testen
3. Rust-Seite in Agent Sessions anpassen
4. App neu bauen: `cd ~/…/agent-sessions && PATH="/usr/local/bin:$HOME/.cargo/bin:/usr/bin:/bin:$PATH" CARGO_HOME="$HOME/.cargo" ./node_modules/.bin/tauri build`
   *(Hinweis: der tauri-Build-Befehl braucht den vollen PATH — sandbox-fähig mit `dangerouslyDisableSandbox: true`)*
5. App neu starten: `pkill -f "Agent Sessions"; open "…/Agent Sessions.app"`
6. Änderungen committen + nach `https://github.com/JonasTietgen/agent-sessions` pushen

---

## Wichtige Details

- Port **7331** — fest verdrahtet, kein Config nötig
- Falls Extension nicht läuft (Port nicht offen): graceful fallback auf bisheriges Verhalten
- Extension muss **nicht** im VS Code Marketplace sein — lokale `.vsix` reicht
- Der Build der Tauri-App läuft nur mit `dangerouslyDisableSandbox: true`
