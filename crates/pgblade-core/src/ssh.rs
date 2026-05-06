use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use crate::connection::{SshAuth, SshConfig};

/// An active SSH tunnel forwarding a local port to a remote host:port.
///
/// The tunnel process is killed when this struct is dropped.
pub struct SshTunnel {
    child: Child,
    local_port: u16,
}

impl SshTunnel {
    /// Establishes an SSH tunnel that forwards `localhost:<free_port>` to
    /// `db_host:db_port` via the SSH server described in `config`.
    ///
    /// Returns the tunnel handle. Use `local_port()` to get the forwarded port.
    ///
    /// Uses `std::process::Command` intentionally: SSH tunnels are long-lived
    /// background processes that must outlive any single async task. The blocking
    /// spawn is acceptable because it returns immediately once the OS starts the process.
    #[allow(clippy::disallowed_methods)]
    pub fn start(config: &SshConfig, db_host: &str, db_port: u16) -> Result<Self, SshTunnelError> {
        let local_port = find_free_port()?;

        let mut cmd = Command::new("ssh");
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());

        // Don't execute a remote command — just forward the port.
        cmd.arg("-N");

        // Local port forwarding: local_port -> db_host:db_port
        cmd.arg("-L")
            .arg(format!("{local_port}:{db_host}:{db_port}"));

        // SSH server port
        cmd.arg("-p").arg(config.port.to_string());

        // Authentication method
        match &config.auth {
            SshAuth::KeyFile { path } => {
                cmd.arg("-i").arg(path);
            }
            SshAuth::Agent => {
                // Default behavior — uses ssh-agent
            }
        }

        // Connection options
        cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");
        cmd.arg("-o").arg("ServerAliveInterval=60");
        cmd.arg("-o").arg("ExitOnForwardFailure=yes");
        cmd.arg("-o").arg("ConnectTimeout=10");

        // user@host
        cmd.arg(format!("{}@{}", config.username, config.host));

        let child = cmd.spawn().map_err(|e| SshTunnelError::SpawnFailed {
            message: e.to_string(),
        })?;

        // Wait for the tunnel to establish by polling the local port.
        wait_for_port(local_port, Duration::from_secs(10))?;

        Ok(Self { child, local_port })
    }

    /// Returns the local port that forwards to the remote database.
    pub fn local_port(&self) -> u16 {
        self.local_port
    }
}

impl Drop for SshTunnel {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Errors that can occur when establishing an SSH tunnel.
#[derive(Debug, thiserror::Error)]
pub enum SshTunnelError {
    #[error("failed to find a free local port: {message}")]
    NoFreePort { message: String },

    #[error("failed to spawn ssh process: {message}")]
    SpawnFailed { message: String },

    #[error("SSH tunnel failed to establish within timeout")]
    Timeout,
}

/// Finds an available local TCP port by binding to port 0.
fn find_free_port() -> Result<u16, SshTunnelError> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| SshTunnelError::NoFreePort {
        message: e.to_string(),
    })?;
    let port = listener
        .local_addr()
        .map_err(|e| SshTunnelError::NoFreePort {
            message: e.to_string(),
        })?
        .port();
    Ok(port)
}

/// Polls until the given local port accepts a TCP connection or the timeout expires.
fn wait_for_port(port: u16, timeout: Duration) -> Result<(), SshTunnelError> {
    let start = std::time::Instant::now();
    let interval = Duration::from_millis(100);

    loop {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return Ok(());
        }
        if start.elapsed() >= timeout {
            return Err(SshTunnelError::Timeout);
        }
        std::thread::sleep(interval);
    }
}
