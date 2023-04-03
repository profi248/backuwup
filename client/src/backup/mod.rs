use tokio::sync::OnceCell;

pub mod filesystem;

static BACKUP_PAUSE: OnceCell<bool> = OnceCell::const_new();

pub async fn networked_backup() -> anyhow::Result<()> {
    Ok(())
}
