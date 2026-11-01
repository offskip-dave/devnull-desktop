//! Register this binary as the `devnull://` handler so GitHub can return after approval.

use super::AuthError;

pub fn register_url_handler() -> Result<(), AuthError> {
    #[cfg(target_os = "linux")]
    {
        linux::register()
    }
    #[cfg(target_os = "macos")]
    {
        macos::register()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Ok(())
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::AuthError;

    pub fn register() -> Result<(), AuthError> {
        if std::env::var_os("FLATPAK_ID").is_some() {
            return Ok(());
        }
        let exe = std::env::current_exe().map_err(|_| AuthError::Handler)?;
        let apps_dir = data_home().join("applications");
        std::fs::create_dir_all(&apps_dir).map_err(|_| AuthError::Handler)?;
        let desktop_path = apps_dir.join("sh.devnull.Client.desktop");
        let desktop = desktop_entry(&quote_exec(&exe));
        std::fs::write(&desktop_path, desktop).map_err(|_| AuthError::Handler)?;

        let _ = std::process::Command::new("update-desktop-database")
            .arg(&apps_dir)
            .status();
        let _ = std::process::Command::new("xdg-mime")
            .args([
                "default",
                "sh.devnull.Client.desktop",
                "x-scheme-handler/devnull",
            ])
            .status();
        let _ = std::process::Command::new("xdg-settings")
            .args([
                "set",
                "default-url-scheme-handler",
                "devnull",
                "sh.devnull.Client.desktop",
            ])
            .status();
        let _ = std::process::Command::new("gio")
            .args([
                "mime",
                "x-scheme-handler/devnull",
                "sh.devnull.Client.desktop",
            ])
            .status();
        write_mimeapps_default();
        Ok(())
    }

    fn data_home() -> std::path::PathBuf {
        xdg_home("XDG_DATA_HOME", ".local/share")
    }

    fn config_home() -> std::path::PathBuf {
        xdg_home("XDG_CONFIG_HOME", ".config")
    }

    fn xdg_home(var: &str, fallback: &str) -> std::path::PathBuf {
        if let Ok(dir) = std::env::var(var) {
            if !dir.is_empty() {
                return std::path::PathBuf::from(dir);
            }
        }
        std::path::PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".into()))
            .join(fallback)
    }

    fn write_mimeapps_default() {
        let path = config_home().join("mimeapps.list");
        let _ = std::fs::create_dir_all(config_home());
        let mut text = std::fs::read_to_string(&path).unwrap_or_default();
        const KEY: &str = "x-scheme-handler/devnull=";
        const LINE: &str = "x-scheme-handler/devnull=sh.devnull.Client.desktop";
        if let Some(idx) = text.find(KEY) {
            let end = text[idx..]
                .find('\n')
                .map(|i| idx + i)
                .unwrap_or(text.len());
            text.replace_range(idx..end, LINE);
        } else if let Some(section) = text.find("[Default Applications]") {
            let insert_at = text[section..]
                .find('\n')
                .map(|i| section + i + 1)
                .unwrap_or(text.len());
            text.insert_str(insert_at, &format!("{LINE}\n"));
        } else {
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(&format!("\n[Default Applications]\n{LINE}\n"));
        }
        let _ = std::fs::write(path, text);
    }

    fn desktop_entry(exec: &str) -> String {
        format!(
            "\
[Desktop Entry]
Type=Application
Name=/dev/null
Comment=A social network for developers
Exec={exec} %u
Icon=sh.devnull.Client
Terminal=false
Categories=Network;InstantMessaging;
MimeType=x-scheme-handler/devnull;
StartupWMClass=sh.devnull.Client
DBusActivatable=false
"
        )
    }

    fn quote_exec(path: &std::path::Path) -> String {
        let raw = path.display().to_string().replace('%', "%%");
        if raw
            .chars()
            .any(|c| c.is_whitespace() || "\"'\\><~|&;$*?#()`".contains(c))
        {
            format!("\"{}\"", raw.replace('\\', "\\\\").replace('"', "\\\""))
        } else {
            raw
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::path::Path;

        #[test]
        fn desktop_entry_passes_the_callback_as_an_argument() {
            let text = desktop_entry("/usr/bin/devnull");
            assert!(text.contains("Exec=/usr/bin/devnull %u"));
            assert!(text.contains("MimeType=x-scheme-handler/devnull;"));
            assert!(text.contains("StartupWMClass=sh.devnull.Client"));
        }

        #[test]
        fn quote_exec_quotes_spaces() {
            assert_eq!(
                quote_exec(Path::new("/home/a b/devnull")),
                "\"/home/a b/devnull\""
            );
            assert_eq!(
                quote_exec(Path::new("/usr/bin/devnull")),
                "/usr/bin/devnull"
            );
        }
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use super::AuthError;

    pub fn register() -> Result<(), AuthError> {
        let exe = std::env::current_exe().map_err(|_| AuthError::Handler)?;
        let support = crate::data_dir();
        std::fs::create_dir_all(&support).map_err(|_| AuthError::Handler)?;
        let callback = support.join("callback.uri");
        let script_path = support.join("url-handler.applescript");
        let bundle = support.join("DevNull Callback.app");
        let script = format!(
            "\
on open location this_URL
  do shell script \"mkdir -p \" & quoted form of {support} & \" && /usr/bin/printf %s \" & quoted form of this_URL & \" > \" & quoted form of {callback}
  do shell script quoted form of {exe} & \" \" & quoted form of this_URL
end open location
",
            exe = applescript_string(&exe.display().to_string()),
            support = applescript_string(&support.display().to_string()),
            callback = applescript_string(&callback.display().to_string()),
        );
        std::fs::write(&script_path, script).map_err(|_| AuthError::Handler)?;
        let status = std::process::Command::new("osacompile")
            .args(["-o"])
            .arg(&bundle)
            .arg(&script_path)
            .status();
        if !status.map(|s| s.success()).unwrap_or(false) {
            return Ok(());
        }
        patch_info_plist(&bundle.join("Contents/Info.plist"));
        let lsregister = "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister";
        let _ = std::process::Command::new(lsregister)
            .args(["-f"])
            .arg(&bundle)
            .status();
        Ok(())
    }

    fn applescript_string(s: &str) -> String {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    }

    fn patch_info_plist(path: &std::path::Path) {
        let Ok(mut text) = std::fs::read_to_string(path) else {
            return;
        };
        if !text.contains("CFBundleURLSchemes") {
            const INSERT: &str = "\
	<key>CFBundleIdentifier</key>
	<string>sh.devnull.Client.url-handler</string>
	<key>CFBundleURLTypes</key>
	<array>
		<dict>
			<key>CFBundleURLName</key>
			<string>sh.devnull.Client</string>
			<key>CFBundleURLSchemes</key>
			<array>
				<string>devnull</string>
			</array>
		</dict>
	</array>
	<key>LSUIElement</key>
	<true/>
";
            if let Some(idx) = text.rfind("</dict>") {
                text.insert_str(idx, INSERT);
            }
        }
        let _ = std::fs::write(path, text);
    }
}
