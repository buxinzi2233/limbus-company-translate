//! Coordinator-only capability files; tokens are never printed to stdout.
use anyhow::{ensure, Context, Result};
use loc_core::{DraftGrantRequest, DraftPermit, Store};
use std::{fs, io::Write, path::Path};

pub fn read_permit(path: &Path) -> Result<DraftPermit> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("Read draft permit {}", path.display()))?;
    ensure!(
        metadata.is_file() && metadata.len() <= 4096,
        "Draft permit must be a regular file of at most 4096 bytes: {}",
        path.display()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        ensure!(
            metadata.permissions().mode() & 0o077 == 0,
            "Draft permit must not be accessible to group/other users; chmod 600 {}",
            path.display()
        );
    }
    serde_json::from_slice(&fs::read(path)?).context("Invalid draft permit JSON")
}

pub fn create_grant(store: &Store, input: &Path, output: &Path) -> Result<()> {
    ensure!(
        fs::metadata(input)?.len() <= 2 * 1024 * 1024,
        "Draft grant request exceeds 2 MiB: {}",
        input.display()
    );
    ensure!(
        !output.exists(),
        "Refusing to replace existing permit file: {}",
        output.display()
    );
    let request: DraftGrantRequest = serde_json::from_slice(&fs::read(input)?).context("Invalid draft grant request; require project_id, worker, manifest_sha256, source_import_id and segment_ids")?;
    let parent = output
        .parent()
        .context("Permit output requires a parent directory")?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    let (receipt, permit) = store.create_draft_grant(&request)?;
    let result = (|| -> Result<()> {
        file.write_all(&serde_json::to_vec(&permit)?)?;
        file.as_file().sync_all()?;
        file.persist_noclobber(output)
            .with_context(|| format!("Publish permit file {}", output.display()))?;
        #[cfg(unix)]
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if let Err(error) = result {
        store.revoke_draft_grant(&request.project_id, &receipt.id)
            .with_context(|| format!("Permit publication failed ({error:#}); also failed to revoke grant {}. Revoke it explicitly before retrying", receipt.id))?;
        return Err(error
            .context("Permit publication failed; grant was revoked, any output file is unusable"));
    }
    println!("{}", serde_json::to_string_pretty(&receipt)?);
    Ok(())
}
