use std::path::{Path, PathBuf};

#[must_use]
pub fn display_version() -> &'static str {
    const RAW: &str = env!("CARGO_PKG_VERSION");
    if let Some(stripped) = RAW.strip_suffix(".0") {
        stripped
    } else {
        RAW
    }
}

#[must_use]
pub fn format_version(version: (u16, u16, u16)) -> String {
    format!("{}.{}.{}", version.0, version.1, version.2)
}

#[must_use]
pub fn is_version_at_least(current: (u16, u16, u16), required: (u16, u16, u16)) -> bool {
    current >= required
}

#[cfg(windows)]
#[must_use]
pub fn read_pe_version(path: &Path) -> Option<(u16, u16, u16)> {
    use std::ffi::c_void;
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::{
        GetFileVersionInfoSizeW, GetFileVersionInfoW, VS_FIXEDFILEINFO, VerQueryValueW,
    };
    use windows::core::PCWSTR;

    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide.push(0);

    let size = unsafe { GetFileVersionInfoSizeW(PCWSTR(wide.as_ptr()), None) };
    if size == 0 {
        return None;
    }

    let mut buffer = vec![0u8; size as usize];
    let ok = unsafe {
        GetFileVersionInfoW(
            PCWSTR(wide.as_ptr()),
            Some(0),
            size,
            buffer.as_mut_ptr().cast(),
        )
    };
    if ok.is_err() {
        return None;
    }

    let sub_block: Vec<u16> = "\\\0".encode_utf16().collect();
    let mut info_ptr: *mut c_void = std::ptr::null_mut();
    let mut info_len: u32 = 0;

    let found = unsafe {
        VerQueryValueW(
            buffer.as_ptr().cast(),
            PCWSTR(sub_block.as_ptr()),
            &raw mut info_ptr,
            &raw mut info_len,
        )
    };
    let base = buffer.as_ptr() as usize;
    let ptr = info_ptr as usize;
    let needed = std::mem::size_of::<VS_FIXEDFILEINFO>();
    if !found.as_bool()
        || info_ptr.is_null()
        || (info_len as usize) < needed
        || ptr < base
        || ptr.saturating_add(needed) > base.saturating_add(buffer.len())
    {
        return None;
    }

    let fixed = unsafe { std::ptr::read_unaligned(info_ptr.cast::<VS_FIXEDFILEINFO>()) };
    let major = ((fixed.dwFileVersionMS >> 16) & 0xFFFF) as u16;
    let minor = (fixed.dwFileVersionMS & 0xFFFF) as u16;
    let patch = ((fixed.dwFileVersionLS >> 16) & 0xFFFF) as u16;
    Some((major, minor, patch))
}

#[cfg(not(windows))]
#[must_use]
pub fn read_pe_version(path: &Path) -> Option<(u16, u16, u16)> {
    read_macos_bundle_version(path)
}

#[must_use]
pub fn read_macos_bundle_version(path: &Path) -> Option<(u16, u16, u16)> {
    for ancestor in path.ancestors().take(4) {
        let plist = if ancestor.ends_with("Contents") {
            ancestor.join("Info.plist")
        } else {
            ancestor.join("Contents").join("Info.plist")
        };
        if let Ok(text) = std::fs::read_to_string(&plist)
            && let Some(ver) = parse_plist_short_version(&text)
        {
            return Some(ver);
        }
    }
    None
}

#[must_use]
pub fn parse_plist_short_version(plist_xml: &str) -> Option<(u16, u16, u16)> {
    let key_idx = plist_xml.find("CFBundleShortVersionString")?;
    let after_key = &plist_xml[key_idx..];
    let start_tag = after_key.find("<string>")?;
    let after_start = &after_key[start_tag + "<string>".len()..];
    let end_tag = after_start.find("</string>")?;
    let raw = after_start[..end_tag].trim().trim_start_matches('v');
    let mut parts = raw.split('.');
    let major = parts.next()?.parse::<u16>().ok()?;
    let minor = parts.next().unwrap_or("0").parse::<u16>().ok()?;
    let patch = parts
        .next()
        .unwrap_or("0")
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .unwrap_or("0")
        .parse::<u16>()
        .ok()?;
    Some((major, minor, patch))
}

#[must_use]
pub fn parse_registry_path_value(raw: &str) -> Option<PathBuf> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let without_quotes = if let Some(rest) = trimmed.strip_prefix('"') {
        rest.split('"').next().unwrap_or(rest)
    } else {
        trimmed
    };
    let clean = match without_quotes.rfind(',') {
        Some(idx)
            if !without_quotes[idx + 1..].is_empty()
                && without_quotes[idx + 1..]
                    .trim()
                    .chars()
                    .all(|c| c.is_ascii_digit() || c == '-') =>
        {
            &without_quotes[..idx]
        }
        _ => without_quotes,
    };
    let clean = clean.trim().trim_matches('"');
    (!clean.is_empty()).then(|| PathBuf::from(clean))
}

#[cfg(windows)]
#[must_use]
pub fn discover_installed_exe(keywords: &[&str], exe_names: &[&str]) -> Option<PathBuf> {
    use windows::Win32::Foundation::ERROR_NO_MORE_ITEMS;
    use windows::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ,
        RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW,
    };
    use windows::core::PCWSTR;

    unsafe fn read_reg_str(hkey: HKEY, name: &str) -> Option<String> {
        let wide_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let mut buf = vec![0u16; 512];
        let mut bytes = (buf.len() * 2) as u32;
        let status = unsafe {
            RegGetValueW(
                hkey,
                PCWSTR::null(),
                PCWSTR(wide_name.as_ptr()),
                RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ,
                None,
                Some(buf.as_mut_ptr().cast()),
                Some(&mut bytes),
            )
        };
        if status.is_err() {
            return None;
        }
        let chars = (bytes as usize / 2).min(buf.len());
        let slice = &buf[..chars];
        let end = slice.iter().position(|&c| c == 0).unwrap_or(slice.len());
        Some(String::from_utf16_lossy(&slice[..end]))
    }

    let roots = [
        (
            HKEY_CURRENT_USER,
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            HKEY_LOCAL_MACHINE,
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            HKEY_LOCAL_MACHINE,
            r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
    ];

    for (hive, subkey_path) in roots {
        let subkey_wide: Vec<u16> = subkey_path
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut root_key = HKEY::default();
        if unsafe {
            RegOpenKeyExW(
                hive,
                PCWSTR(subkey_wide.as_ptr()),
                Some(0),
                KEY_READ,
                &mut root_key,
            )
        }
        .is_err()
        {
            continue;
        }

        let mut index = 0u32;
        let mut name_buf = [0u16; 256];
        loop {
            let mut name_len = name_buf.len() as u32;
            let status = unsafe {
                RegEnumKeyExW(
                    root_key,
                    index,
                    Some(windows::core::PWSTR(name_buf.as_mut_ptr())),
                    &mut name_len,
                    None,
                    None,
                    None,
                    None,
                )
            };
            if status == ERROR_NO_MORE_ITEMS || status.is_err() {
                break;
            }
            index += 1;

            let child_name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
            let child_wide: Vec<u16> = child_name
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let mut app_key = HKEY::default();
            if unsafe {
                RegOpenKeyExW(
                    root_key,
                    PCWSTR(child_wide.as_ptr()),
                    Some(0),
                    KEY_READ,
                    &mut app_key,
                )
            }
            .is_err()
            {
                continue;
            }

            let display_name = unsafe { read_reg_str(app_key, "DisplayName") }
                .unwrap_or_default()
                .to_ascii_lowercase();
            let key_lower = child_name.to_ascii_lowercase();
            let matched = keywords
                .iter()
                .any(|kw| display_name.contains(kw) || key_lower.contains(kw));
            if matched {
                if let Some(icon) = unsafe { read_reg_str(app_key, "DisplayIcon") }
                    .and_then(|v| parse_registry_path_value(&v))
                    && icon.is_file()
                    && exe_names.iter().any(|e| {
                        icon.file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| n.eq_ignore_ascii_case(e))
                    })
                {
                    let _ = unsafe { RegCloseKey(app_key) }; // ignore-ok: close app uninstall key
                    let _ = unsafe { RegCloseKey(root_key) }; // ignore-ok: close uninstall root key
                    return Some(icon);
                }

                for field in ["InstallLocation", "UninstallString"] {
                    let Some(raw) = (unsafe { read_reg_str(app_key, field) }) else {
                        continue;
                    };
                    let Some(parsed) = parse_registry_path_value(&raw) else {
                        continue;
                    };
                    let dir = if field == "UninstallString" {
                        parsed.parent().map(Path::to_path_buf)
                    } else {
                        Some(parsed)
                    };
                    if let Some(dir) = dir {
                        for exe in exe_names {
                            let candidate = dir.join(exe);
                            if candidate.is_file() {
                                let _ = unsafe { RegCloseKey(app_key) }; // ignore-ok: close app uninstall key
                                let _ = unsafe { RegCloseKey(root_key) }; // ignore-ok: close uninstall root key
                                return Some(candidate);
                            }
                        }
                    }
                }
            }
            let _ = unsafe { RegCloseKey(app_key) }; // ignore-ok: close app uninstall key
        }
        let _ = unsafe { RegCloseKey(root_key) }; // ignore-ok: close uninstall root key
    }
    None
}

#[cfg(not(windows))]
#[must_use]
pub fn discover_installed_exe(_keywords: &[&str], _exe_names: &[&str]) -> Option<PathBuf> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_version() {
        let v = display_version();
        assert!(!v.is_empty());
        assert!(v.contains('.'));
    }

    #[test]
    fn test_version_comparison() {
        assert!(is_version_at_least((1, 33, 0), (1, 30, 0)));
        assert!(is_version_at_least((1, 30, 0), (1, 30, 0)));
        assert!(!is_version_at_least((1, 29, 9), (1, 30, 0)));
        assert_eq!(format_version((1, 33, 0)), "1.33.0");
    }

    #[cfg(windows)]
    #[test]
    fn test_read_pe_version_from_notepad() {
        let notepad = Path::new(r"C:\Windows\System32\notepad.exe");
        if notepad.is_file() {
            let ver = read_pe_version(notepad);
            assert!(ver.is_some());
        }
    }

    #[test]
    fn parses_quoted_install_location_and_icon_suffixes() {
        assert_eq!(
            parse_registry_path_value(r#""D:\Apps\LTK Manager""#),
            Some(PathBuf::from(r"D:\Apps\LTK Manager"))
        );
        assert_eq!(
            parse_registry_path_value(r#""D:\Apps\LTK Manager\ltk-manager.exe",0"#),
            Some(PathBuf::from(r"D:\Apps\LTK Manager\ltk-manager.exe"))
        );
        assert_eq!(
            parse_registry_path_value(r"C:\Program Files\LTK Manager\ltk-manager.exe"),
            Some(PathBuf::from(
                r"C:\Program Files\LTK Manager\ltk-manager.exe"
            ))
        );
        assert_eq!(parse_registry_path_value("   "), None);
        let plist = "<plist><dict><key>CFBundleShortVersionString</key><string>1.34.2</string></dict></plist>";
        assert_eq!(parse_plist_short_version(plist), Some((1, 34, 2)));
    }
}
