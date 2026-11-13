//! Capture the authorized source once. Later searches and reads use owned text
//! from this snapshot. Provider input never reaches a file handle or shell.
//! The caller supplies the reviewed roots and checks disclosure before dispatch.
use std::{path::Path, sync::atomic::AtomicBool, time::Instant};

pub(crate) struct SourceFile {
    pub path: String,
    pub text: String,
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::{fs, time::Duration};

    fn read(root: &Path, paths: &[&str]) -> Result<SourceSnapshot, String> {
        let targets = paths.iter().map(|path| (*path).to_owned()).collect::<Vec<_>>();
        capture(root, &targets, &AtomicBool::new(false), Instant::now() + Duration::from_secs(15))
    }

    #[test]
    fn only_owned_text_from_selected_paths_is_available_after_capture() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        let src = project.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("a.rs"), "analyze me\n").unwrap();
        fs::write(project.join("sibling.txt"), "private").unwrap();
        fs::write(dir.path().join("outside.txt"), "outside").unwrap();
        let snapshot = read(&project, &["src", "src/a.rs"]).unwrap();
        fs::write(src.join("a.rs"), "new data").unwrap();
        assert_eq!(snapshot.files.len(), 1);
        assert_eq!(snapshot.files[0].path, "src/a.rs");
        assert_eq!(snapshot.files[0].text, "analyze me\n");
        assert_eq!(snapshot.omitted, 0);
        for escape in ["../outside.txt", "src/../../outside.txt", "C:/outside.txt", r"\\host\share", "a.rs:secret"] {
            assert!(read(&project, &[escape]).is_err(), "escape accepted");
        }
    }

    #[test]
    fn skips_dependency_trees_credential_files_binary_and_oversized_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("node_modules")).unwrap();
        fs::write(dir.path().join("node_modules/never.txt"), "hidden").unwrap();
        fs::write(dir.path().join(".env.local"), "hidden").unwrap();
        fs::write(dir.path().join("api_openrouter.txt"), "hidden").unwrap();
        fs::write(dir.path().join("image.dat"), [0, 1, 2]).unwrap();
        fs::write(dir.path().join("large.txt"), vec![b'x'; (crate::source_scan::MAX_SCAN_FILE_BYTES + 1) as usize]).unwrap();
        fs::write(dir.path().join("code.py"), "print('public')").unwrap();
        let snapshot = read(dir.path(), &[]).unwrap();
        assert_eq!(snapshot.files.len(), 1);
        assert!(snapshot.files[0].text.contains("public"));
        assert_eq!(snapshot.omitted, 5);
        assert!(!snapshot.excluded_paths.iter().any(|p| p.contains("never")));
        assert!(read(dir.path(), &["node_modules/never.txt"]).unwrap().files.is_empty());
    }

    #[test]
    fn junction_hardlink_and_an_external_project_root_alias_cannot_disclose_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("project");
        let outside = dir.path().join("outside");
        fs::create_dir(&project).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("private.txt"), "synthetic external secret").unwrap();
        fs::write(project.join("ok.rs"), "ok").unwrap();
        fs::hard_link(outside.join("private.txt"), project.join("alias.rs")).unwrap();
        let junction = project.join("mount");
        let status = std::process::Command::new("cmd.exe").args(["/d", "/c", "mklink", "/J"])
            .arg(&junction).arg(&outside).output().unwrap();
        assert!(status.status.success());
        let snapshot = read(&project, &[]).unwrap();
        assert_eq!(snapshot.files.len(), 1);
        assert_eq!(snapshot.files[0].path, "ok.rs");
        assert_eq!(snapshot.omitted, 2);
        assert!(read(&project, &["mount/private.txt"]).is_err());
        assert!(read(&project, &["alias.rs"]).is_err());
        assert!(read(&junction, &[]).is_err());
        fs::remove_dir(junction).unwrap();
        assert_eq!(fs::read_to_string(outside.join("private.txt")).unwrap(), "synthetic external secret");
    }

    #[test]
    fn cancellation_and_expiry_prevent_capture() {
        let dir = tempfile::tempdir().unwrap();
        assert!(capture(dir.path(), &[], &AtomicBool::new(true), Instant::now() + Duration::from_secs(1)).is_err());
        assert!(capture(dir.path(), &[], &AtomicBool::new(false), Instant::now()).is_err());
    }

    #[test]
    fn explicit_targets_and_project_roots_cannot_bypass_credential_directory_exclusions() {
        let dir = tempfile::tempdir().unwrap();
        let key_store = dir.path().join(".ssh");
        fs::create_dir(&key_store).unwrap();
        fs::write(key_store.join("misleading.rs"), "synthetic private material").unwrap();
        assert!(read(&key_store, &[]).is_err());
        assert!(read(dir.path(), &[".ssh/misleading.rs"]).unwrap().files.is_empty());
    }

    #[test]
    fn local_connection_and_service_account_material_stays_out_of_source_snapshots() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let private_files = [
            "VNC.txt", "SERVIDOR_IP.txt", "service-account-staging.json",
            "App-Firebase-AdminSDK-local.json", "vault.dpapi", "identity.jks",
            ".lab-local/pairing/private.txt", "secrets/production/config.json",
        ];
        for relative in private_files {
            let path = root.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "private synthetic canary").unwrap();
        }
        fs::write(root.join("source.rs"), "pub fn hello() {}").unwrap();
        let snapshot = read(root, &[]).unwrap();
        assert_eq!(snapshot.files.iter().map(|f| f.path.as_str()).collect::<Vec<_>>(), ["source.rs"]);
        for relative in private_files {
            assert!(read(root, &[relative]).unwrap().files.is_empty(), "{relative}");
        }
        for relative in [".lab-local/pairing", "secrets/production"] {
            assert!(read(&root.join(relative), &[]).is_err(), "{relative}");
        }
    }
}

#[derive(Default)]
pub(crate) struct SourceSnapshot {
    pub files: Vec<SourceFile>,
    /// Each skipped file or untraversed directory counts as one entry.
    pub omitted: usize,
    pub excluded_paths: Vec<String>,
}

#[cfg(not(windows))]
pub(crate) fn capture(_: &Path, _: &[String], _: &AtomicBool, _: Instant) -> Result<SourceSnapshot, String> {
    Err("A leitura isolada exige Windows.".into())
}

#[cfg(windows)]
pub(crate) use windows_capture::capture;

#[cfg(windows)]
mod windows_capture {
    use super::*;
    use std::{
        collections::HashSet, fs::File, io::Read, mem::{offset_of, size_of},
        os::windows::{ffi::OsStringExt, fs::{MetadataExt, OpenOptionsExt}, io::{AsRawHandle, FromRawHandle}},
        path::{Component, Prefix}, ptr, sync::atomic::Ordering,
    };
    use windows_sys::{
        Wdk::{Foundation::OBJECT_ATTRIBUTES, Storage::FileSystem::{
            NtCreateFile, FILE_OPEN, FILE_OPEN_REPARSE_POINT, FILE_SYNCHRONOUS_IO_NONALERT,
        }},
        Win32::{Foundation::{ERROR_NO_MORE_FILES, OBJ_CASE_INSENSITIVE, OBJ_DONT_REPARSE, UNICODE_STRING},
            Storage::FileSystem::*, System::IO::IO_STATUS_BLOCK},
    };

    const MAX_FILES: usize = crate::source_scan::MAX_FILES;
    const MAX_BYTES: usize = crate::source_scan::MAX_INPUT_BYTES as usize;
    const MAX_FILE: usize = crate::source_scan::MAX_SCAN_FILE_BYTES as usize;
    const MAX_NODES: usize = 25_000;
    const MAX_DEPTH: usize = 48;
    const UNAVAILABLE: &str = "Não foi possível verificar a entrada dentro da pasta autorizada.";

    struct Reader<'a> {
        root: &'a Path,
        cancel: &'a AtomicBool,
        deadline: Instant,
        snapshot: SourceSnapshot,
        seen: HashSet<String>,
        bytes: usize,
        nodes: usize,
    }

    pub(crate) fn capture(repo: &Path, targets: &[String], cancel: &AtomicBool, deadline: Instant) -> Result<SourceSnapshot, String> {
        if targets.len() > 64 {
            return Err("Quantidade de alvos acima do limite da análise.".into());
        }
        let mut reader = Reader { root: repo, cancel, deadline, snapshot: SourceSnapshot::default(), seen: HashSet::new(), bytes: 0, nodes: 0 };
        reader.checkpoint()?;
        // Anchor at a disk handle. Open each subsequent component relative to
        // its already verified parent with reparse processing disabled.
        let mut parts = repo.components();
        let Some(Component::Prefix(prefix)) = parts.next() else { return Err(UNAVAILABLE.into()) };
        let drive = match prefix.kind() {
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => letter,
            _ => return Err(UNAVAILABLE.into()),
        };
        if !matches!(parts.next(), Some(Component::RootDir)) { return Err(UNAVAILABLE.into()); }
        let drive_root = format!("{}:\\", char::from(drive));
        let mut root_handle = std::fs::OpenOptions::new().read(true).share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT).open(&drive_root).map_err(|_| UNAVAILABLE)?;
        if root_handle.metadata().map_err(|_| UNAVAILABLE)?.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(UNAVAILABLE.into());
        }
        let mut expected = std::path::PathBuf::from(&drive_root);
        let mut root_guards = Vec::new();
        for part in parts {
            let Component::Normal(part) = part else { return Err(UNAVAILABLE.into()) };
            let name = part.to_str().ok_or(UNAVAILABLE)?;
            if credential_directory(&name.to_ascii_lowercase()) {
                return Err("O repositório está em uma pasta de credenciais ou metadados privados.".into());
            }
            expected.push(name);
            let (next, directory) = open_child(&root_handle, name, &expected, true)?;
            if !directory { return Err(UNAVAILABLE.into()); }
            root_guards.push(root_handle);
            root_handle = next;
        }
        if targets.is_empty() {
            reader.directory(&root_handle, "", 0)?;
        } else {
            for target in targets {
                reader.checkpoint()?;
                let target = crate::scope_targets::normalize_relative(target)?;
                if target.len() > 1024 || target.split('/').count() > MAX_DEPTH {
                    return Err(UNAVAILABLE.into());
                }
                if target.split('/').any(excluded_name) {
                    reader.omit(&target);
                    continue;
                }
                let mut parent = root_handle.try_clone().map_err(|_| UNAVAILABLE)?;
                let mut guards = Vec::new();
                let mut relative = String::new();
                let mut components = target.split('/').peekable();
                while let Some(name) = components.next() {
                    if !relative.is_empty() { relative.push('/'); }
                    relative.push_str(name);
                    let (child, directory) = open_child(&parent, name, &repo.join(&relative), components.peek().is_some())?;
                    if components.peek().is_none() {
                        reader.visit(child, directory, &relative, relative.split('/').count())?;
                        break;
                    }
                    if !directory { return Err(UNAVAILABLE.into()); }
                    guards.push(parent);
                    parent = child;
                }
            }
        }
        reader.snapshot.files.sort_by(|a,b| a.path.cmp(&b.path));
        Ok(reader.snapshot)
    }

    // The filename is a single UTF-16 component, never a path expression. A
    // single verified handle supplies type, name and content for its lifetime.
    fn open_child(parent: &File, name: &str, expected: &Path, directory_hint: bool) -> Result<(File, bool), String> {
        if name.contains(['/', '\\']) || crate::scope_targets::normalize_relative(name).is_err() {
            return Err(UNAVAILABLE.into());
        }
        let mut wide: Vec<u16> = name.encode_utf16().collect();
        if wide.len() > 255 { return Err(UNAVAILABLE.into()); }
        let len = (wide.len() * 2) as u16;
        let object_name = UNICODE_STRING { Buffer: wide.as_mut_ptr(), Length: len, MaximumLength: len };
        let attributes = OBJECT_ATTRIBUTES { Length: size_of::<OBJECT_ATTRIBUTES>() as u32,
            RootDirectory: parent.as_raw_handle(), ObjectName: &object_name,
            Attributes: OBJ_CASE_INSENSITIVE | OBJ_DONT_REPARSE, ..Default::default() };
        let mut io = IO_STATUS_BLOCK::default();
        let mut handle = ptr::null_mut();
        // Directory ancestors stay shareable for ordinary writes. The relative
        // opens keep children anchored even during an attempted namespace swap.
        let share = if directory_hint { FILE_SHARE_READ | FILE_SHARE_WRITE } else { FILE_SHARE_READ };
        // SAFETY: all pointers and the parent handle remain valid through the call.
        let status = unsafe { NtCreateFile(&mut handle, FILE_READ_ATTRIBUTES | FILE_READ_DATA | SYNCHRONIZE,
            &attributes, &mut io, ptr::null(), 0, share, FILE_OPEN,
            FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT, ptr::null(), 0) };
        if status < 0 || handle.is_null() { return Err(UNAVAILABLE.into()); }
        // SAFETY: successful NtCreateFile transferred one owned handle.
        let file = unsafe { File::from_raw_handle(handle) };
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        if unsafe { GetFileInformationByHandle(handle, &mut info) } == 0 || info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(UNAVAILABLE.into());
        }
        let directory = info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0;
        if (directory_hint && !directory) || (!directory && info.nNumberOfLinks != 1) { return Err(UNAVAILABLE.into()); }
        let mut buf = vec![0u16; 32768];
        let size = unsafe { GetFinalPathNameByHandleW(handle, buf.as_mut_ptr(), buf.len() as u32, 0) } as usize;
        if size == 0 || size >= buf.len() { return Err(UNAVAILABLE.into()); }
        let effective = std::ffi::OsString::from_wide(&buf[..size]);
        let normalize = |value: &Path| value.to_string_lossy().trim_start_matches(r"\\?\").replace('/', "\\").to_lowercase();
        if normalize(Path::new(&effective)) != normalize(expected) { return Err(UNAVAILABLE.into()); }
        Ok((file, directory))
    }

    impl Reader<'_> {
        fn checkpoint(&self) -> Result<(), String> {
            if self.cancel.load(Ordering::Acquire) { return Err("Leitura cancelada.".into()); }
            if Instant::now() >= self.deadline { return Err("Tempo de leitura esgotado.".into()); }
            Ok(())
        }

        fn omit(&mut self, path: &str) {
            self.snapshot.omitted += 1;
            if self.snapshot.excluded_paths.len() < 128 {
                self.snapshot.excluded_paths.push(path.to_owned());
            }
        }

        fn visit(&mut self, file: File, directory: bool, relative: &str, depth: usize) -> Result<(), String> {
            self.checkpoint()?;
            if !self.seen.insert(relative.to_owned()) { return Ok(()); }
            if directory {
                return self.directory(&file, relative, depth);
            }
            let size = file.metadata().map_err(|_| UNAVAILABLE)?.len();
            if size > MAX_FILE as u64 || size > MAX_BYTES.saturating_sub(self.bytes) as u64 || self.snapshot.files.len() >= MAX_FILES {
                self.omit(relative);
                return Ok(());
            }
            let mut bytes = Vec::with_capacity(size as usize);
            file.take(size + 1).read_to_end(&mut bytes).map_err(|_| UNAVAILABLE)?;
            if bytes.len() as u64 != size { return Err("O tamanho de um arquivo mudou durante a leitura.".into()); }
            self.bytes += bytes.len();
            if !bytes.iter().any(|b| *b < 32 && !matches!(*b,b'\r'|b'\n'|b'\t'|0x0c)) {
                if let Ok(text) = String::from_utf8(bytes) {
                    // These bytes stay in native memory. The dispatcher applies the
                    // same live credential redactor to paths, content and results.
                    self.snapshot.files.push(SourceFile { path: relative.to_owned(), text });
                    return Ok(());
                }
            }
            self.omit(relative);
            Ok(())
        }

        fn directory(&mut self, directory: &File, relative: &str, depth: usize) -> Result<(), String> {
            self.checkpoint()?;
            if depth >= MAX_DEPTH {
                self.omit(relative);
                return Ok(());
            }
            let entries = entries(directory, MAX_NODES.saturating_sub(self.nodes))?;
            self.nodes += entries.len();
            for (name, flags) in entries {
                self.checkpoint()?;
                let path = if relative.is_empty() { name.clone() } else { format!("{relative}/{name}") };
                if excluded_name(&name) || path.len() > 1024 || flags & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                    self.omit(&path);
                    continue;
                }
                let child_is_dir = flags & FILE_ATTRIBUTE_DIRECTORY != 0;
                if self.seen.contains(&path) { continue; }
                // Inaccessible, renamed and multiply linked files are omissions.
                // No path-based reopen happens after admission.
                match open_child(directory, &name, &self.root.join(&path), child_is_dir) {
                    Ok((file, directory)) => self.visit(file, directory, &path, depth + 1)?,
                    Err(_) => self.omit(&path),
                }
            }
            Ok(())
        }
    }

    // Enumerate the same open directory handle we validated. read_dir(path)
    // would resolve a mutable pathname again and could follow a new junction.
    fn entries(directory: &File, remaining: usize) -> Result<Vec<(String,u32)>,String> {
        let mut entries = Vec::new();
        let mut buffer = vec![0u64; 8192]; // aligned FILE_FULL_DIR_INFO records
        let capacity = buffer.len() * size_of::<u64>();
        let mut class = FileFullDirectoryRestartInfo;
        loop {
            let ok = unsafe { GetFileInformationByHandleEx(directory.as_raw_handle(), class, buffer.as_mut_ptr().cast(), capacity as u32) };
            if ok == 0 {
                if std::io::Error::last_os_error().raw_os_error() == Some(ERROR_NO_MORE_FILES as i32) { break; }
                return Err(UNAVAILABLE.into());
            }
            class = FileFullDirectoryInfo;
            let mut pos = 0;
            loop {
                if pos + size_of::<FILE_FULL_DIR_INFO>() > capacity || pos % 8 != 0 { return Err(UNAVAILABLE.into()); }
                let start = pos + offset_of!(FILE_FULL_DIR_INFO, FileName);
                // SAFETY: storage is aligned and contains a complete record header.
                let header = unsafe { &*buffer.as_ptr().cast::<u8>().add(pos).cast::<FILE_FULL_DIR_INFO>() };
                let length = header.FileNameLength as usize;
                if length == 0 || length % 2 != 0 || start + length > capacity { return Err(UNAVAILABLE.into()); }
                let name = unsafe { std::slice::from_raw_parts(buffer.as_ptr().cast::<u8>().add(start).cast::<u16>(), length/2) };
                let name = String::from_utf16(name).map_err(|_| UNAVAILABLE)?;
                if name != "." && name != ".." {
                    if entries.len() >= remaining { return Err("Árvore excede 25.000 entradas. Selecione uma pasta menor.".into()); }
                    entries.push((name, header.FileAttributes));
                }
                if header.NextEntryOffset == 0 { break; }
                let next = pos + header.NextEntryOffset as usize;
                if next < start + length || next >= capacity { return Err(UNAVAILABLE.into()); }
                pos = next;
            }
        }
        entries.sort_by(|a,b| a.0.cmp(&b.0));
        Ok(entries)
    }

    fn credential_directory(lower: &str) -> bool {
        matches!(lower, ".git"|".hg"|".svn"|".ssh"|".aws"|".gnupg"|".azure"|".kube"|".codex"|".lab-local"|"secrets")
    }

    fn excluded_name(name: &str) -> bool {
        let lower = name.to_ascii_lowercase();
        credential_directory(&lower) || matches!(lower.as_str(), ".local"|".npmrc"|".pypirc"|".netrc"|"_netrc"|"credentials"|"credentials.json"|
            "id_rsa"|"id_dsa"|"id_ed25519"|"id_ecdsa"|"api_open_ai.txt"|"api_openrouter.txt"|"vnc.txt"|"servidor_ip.txt"|
            "node_modules"|"target"|".venv"|"__pycache__"|".next"|"dist"|"build"|"coverage") ||
            lower == ".env" || lower.starts_with(".env.") ||
            (lower.ends_with(".json") && (lower.starts_with("service-account") || lower.contains("firebase-adminsdk"))) ||
            matches!(lower.rsplit('.').next(),Some("key"|"pem"|"pfx"|"p12"|"jks"|"dpapi"|"kdbx"|"keystore"|"sqlite"|"sqlite3"|"db"|"exe"|"dll"|"zip"|"gz"|"png"|"jpg"|"jpeg"|"ico"|"gif"|"webp"|"pdf"|"mp4"|"woff"|"woff2"))
    }
}
