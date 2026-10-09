//! R1 ServerProcess: start (or attach to) the AIOven server and expose its base URL.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use url::Url;

pub struct Server {
    pub base_url: Url,
    pub directory: PathBuf,
    /// keeps the spawned server alive; `kill_on_drop` stops it when the client exits
    _child: Option<Child>,
}

impl Server {
    /// Spawn `aioven serve --port 0` in the project directory and wait for "listening on http://…".
    pub async fn start(project: &Path) -> Result<Server> {
        let program = std::env::var("AIOVEN_BIN").unwrap_or_else(|_| "aioven".into());
        let mut child = Command::new(&program)
            .args(["serve", "--port", "0", "--hostname", "127.0.0.1"])
            .current_dir(project)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| format!("could not start `{program} serve` (set AIOVEN_BIN or put aioven on PATH)"))?;
        let stdout = child.stdout.take().ok_or_else(|| anyhow!("server stdout unavailable"))?;
        let mut lines = BufReader::new(stdout).lines();
        let url = tokio::time::timeout(Duration::from_secs(60), async {
            while let Some(line) = lines.next_line().await? {
                if let Some(i) = line.find("http://") {
                    return Ok::<_, anyhow::Error>(Url::parse(line[i..].trim())?);
                }
            }
            bail!("server exited before it was ready")
        })
        .await
        .context("server did not start within 60s")??;
        // keep draining stdout so the server never blocks on a full pipe
        tokio::spawn(async move { while let Ok(Some(_)) = lines.next_line().await {} });
        Ok(Server { base_url: url, directory: project.to_path_buf(), _child: Some(child) })
    }

    pub fn attach(url: Url, project: &Path) -> Server {
        Server { base_url: url, directory: project.to_path_buf(), _child: None }
    }
}
