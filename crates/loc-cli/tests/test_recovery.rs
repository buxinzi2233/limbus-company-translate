use anyhow::Result;
use loc_cli::commands::{run_inject, run_restore};
use loc_core::Store;
use serde_json::{json, Value};
use std::{fs, path::Path};
use tempfile::tempdir;

fn ticket(path: &Path, names: &[&str]) -> Result<()> {
    let lines = names
        .iter()
        .map(|file| {
            serde_json::to_string(
                &json!({"file":file,"pointer":"/text","source_kr":"안녕하세요","target_zh":"你好"}),
            )
        })
        .collect::<serde_json::Result<Vec<_>>>()?;
    fs::write(path, lines.join("\n"))?;
    Ok(())
}

#[test]
fn restores_exact_bytes_new_files_and_rejects_unrelated_edits_or_corrupt_backups() -> Result<()> {
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    for name in ["a.json", "z.json"] {
        fs::write(kr.join(name), r#"{"text":"안녕하세요"}"#)?;
    }
    let original = b"{ \"text\" : \"old\" }\n";
    fs::write(zh.join("a.json"), original)?;
    let input = root.path().join("ticket.jsonl");
    ticket(&input, &["a.json", "z.json"])?;
    let summary = run_inject(
        &Store::open_in_memory()?,
        &zh,
        Some(&kr),
        Some(&input),
        true,
        false,
    )?;
    let backup = summary.backup_directory.unwrap();
    let injected = fs::read(zh.join("a.json"))?;
    fs::write(zh.join("z.json"), "unrelated edit")?;
    assert!(run_restore(&zh, &backup).is_err());
    assert_eq!(fs::read(zh.join("a.json"))?, injected);
    fs::write(zh.join("z.json"), &injected)?;
    fs::write(backup.join("originals/a.json"), "corrupt backup")?;
    assert!(run_restore(&zh, &backup).is_err());
    assert_eq!(fs::read(zh.join("a.json"))?, injected);
    fs::write(backup.join("originals/a.json"), original)?;
    let restored = run_restore(&zh, &backup)?;
    assert_eq!(restored.restored_files, 1);
    assert_eq!(restored.removed_files, 1);
    assert_eq!(fs::read(zh.join("a.json"))?, original);
    assert!(!zh.join("z.json").exists());
    assert!(!zh.join(".lct-inject.lock").exists());
    assert_eq!(run_restore(&zh, &backup)?.restored_files, 0);
    Ok(())
}

#[test]
fn recovery_rejects_running_writer_and_malicious_manifest() -> Result<()> {
    let root = tempdir()?;
    let zh = root.path();
    let backup = zh
        .join(".lct-backups")
        .join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(&backup)?;
    fs::write(
        backup.join("manifest.json"),
        serde_json::to_vec(
            &json!({"version":1,"workspace":zh,"entries":[{"file":"../outside.json","existed":false,"original_sha256":null,"replacement_sha256":"bad"}]}),
        )?,
    )?;
    let guard = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(zh.join(".lct-writer.lock"))?;
    guard.try_lock()?;
    assert!(format!("{:#}", run_restore(zh, &backup).err().unwrap()).contains("Another injection"));
    drop(guard);
    assert!(format!("{:#}", run_restore(zh, &backup).err().unwrap()).contains("Invalid relative"));
    Ok(())
}

#[cfg(unix)]
#[test]
fn killed_real_cli_batch_is_recoverable_without_partial_translations() -> Result<()> {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    let root = tempdir()?;
    let kr = root.path().join("kr");
    let zh = root.path().join("zh");
    fs::create_dir(&kr)?;
    fs::create_dir(&zh)?;
    let names: Vec<String> = (0..1000).map(|i| format!("{i:04}.json")).collect();
    for name in &names {
        fs::write(kr.join(name), r#"{"text":"안녕하세요"}"#)?;
        fs::write(zh.join(name), r#"{"text":"original"}"#)?;
    }
    let input = root.path().join("ticket.jsonl");
    ticket(
        &input,
        &names.iter().map(String::as_str).collect::<Vec<_>>(),
    )?;
    let mut child = Command::new(env!("CARGO_BIN_EXE_lct"))
        .arg("--data-dir")
        .arg(root.path().join("data"))
        .arg("inject")
        .arg("--workspace")
        .arg(&zh)
        .arg("--kr")
        .arg(&kr)
        .arg("--ticket")
        .arg(&input)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()?;
    let start = Instant::now();
    loop {
        if fs::read_to_string(zh.join(&names[0]))?.contains("你好") {
            child.kill()?;
            break;
        }
        if let Some(status) = child.try_wait()? {
            anyhow::bail!("CLI exited before interruption: {status}");
        }
        if start.elapsed() > Duration::from_secs(30) {
            child.kill()?;
            child.wait()?;
            anyhow::bail!("Timed out waiting for first published target");
        }
        std::thread::sleep(Duration::from_micros(100));
    }
    assert!(!child.wait()?.success());
    let published = names
        .iter()
        .filter(|name| fs::read_to_string(zh.join(name)).unwrap().contains("你好"))
        .count();
    assert!(
        published > 0 && published < names.len(),
        "Expected a genuinely interrupted batch, got {published}"
    );
    let lock: Value = serde_json::from_slice(&fs::read(zh.join(".lct-inject.lock"))?)?;
    let backup = Path::new(lock["backup"].as_str().unwrap());
    assert_eq!(run_restore(&zh, backup)?.restored_files, published);
    for name in &names {
        assert_eq!(fs::read_to_string(zh.join(name))?, r#"{"text":"original"}"#);
    }
    assert!(!zh.join(".lct-inject.lock").exists());
    Ok(())
}

#[test]
fn restore_clears_only_its_own_unpublished_operation() -> Result<()> {
    let root = tempdir()?;
    let workspace = root.path();
    let backup = workspace
        .join(".lct-backups")
        .join(uuid::Uuid::new_v4().to_string());
    let other = workspace
        .join(".lct-backups")
        .join(uuid::Uuid::new_v4().to_string());
    fs::write(workspace.join("a.json"), "original bytes")?;
    fs::write(
        workspace.join(".lct-inject.lock"),
        serde_json::to_vec(&json!({"backup":backup}))?,
    )?;
    assert!(run_restore(workspace, &other).is_err());
    assert!(workspace.join(".lct-inject.lock").exists());
    let restored = run_restore(workspace, &backup)?;
    assert!(restored.interrupted_before_publication);
    assert_eq!(
        fs::read_to_string(workspace.join("a.json"))?,
        "original bytes"
    );
    assert!(!workspace.join(".lct-inject.lock").exists());
    Ok(())
}
