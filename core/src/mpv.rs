use crate::types::PlaylistItem;
use anyhow::{Context, Result};
use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::net::UnixStream;

#[cfg(windows)]
use interprocess::local_socket::LocalSocketStream;

/// mpv client using IPC socket for instant video switching
pub struct MpvClient {
    socket_path: PathBuf,
    process: Mutex<Option<Child>>,
    pub connect_timeout: Duration,
    command_timeout: Duration,
}

trait IpcStreamTrait: Read + Write {
    fn set_read_timeout_custom(&self, duration: Option<Duration>) -> Result<()>;
    fn set_write_timeout_custom(&self, duration: Option<Duration>) -> Result<()>;
}

#[cfg(unix)]
impl IpcStreamTrait for UnixStream {
    fn set_read_timeout_custom(&self, duration: Option<Duration>) -> Result<()> {
        self.set_read_timeout(duration)
            .context("Failed to set read timeout")
    }
    fn set_write_timeout_custom(&self, duration: Option<Duration>) -> Result<()> {
        self.set_write_timeout(duration)
            .context("Failed to set write timeout")
    }
}

#[cfg(windows)]
impl IpcStreamTrait for LocalSocketStream {
    fn set_read_timeout_custom(&self, _duration: Option<Duration>) -> Result<()> {
        // Not supported on Windows LocalSocketStream cleanly without async or raw handle hacking
        Ok(())
    }
    fn set_write_timeout_custom(&self, _duration: Option<Duration>) -> Result<()> {
        Ok(())
    }
}

impl MpvClient {
    #[must_use]
    pub fn new_unix(
        socket_path: String,
        connect_timeout_secs: f64,
        command_timeout_secs: f64,
    ) -> Self {
        Self {
            socket_path: PathBuf::from(socket_path),
            process: Mutex::new(None),
            connect_timeout: Duration::from_secs_f64(connect_timeout_secs),
            command_timeout: Duration::from_secs_f64(command_timeout_secs),
        }
    }

    fn connect(&self) -> Result<Box<dyn IpcStreamTrait>> {
        #[cfg(unix)]
        {
            let stream = UnixStream::connect(&self.socket_path)
                .context("Failed to connect to mpv socket")?;
            Ok(Box::new(stream))
        }
        #[cfg(windows)]
        {
            // On Windows, the socket path from config is likely a named pipe path string: \\.\pipe\name
            // interprocess expects a name usually.
            let path_str = self.socket_path.to_string_lossy();
            let pipe_name = if path_str.starts_with(r"\\.\pipe\") {
                &path_str[9..]
            } else {
                &path_str
            };

            let stream =
                LocalSocketStream::connect(pipe_name).context("Failed to connect to named pipe")?;
            Ok(Box::new(stream))
        }
    }

    fn is_daemon_alive_internal(&self) -> bool {
        #[cfg(unix)]
        return self.socket_path.exists() && UnixStream::connect(&self.socket_path).is_ok();

        #[cfg(windows)]
        {
            // On Windows, checking if pipe exists is tricky without connecting.
            // Just try connecting.
            let path_str = self.socket_path.to_string_lossy();
            let pipe_name = if path_str.starts_with(r"\\.\pipe\") {
                &path_str[9..]
            } else {
                &path_str
            };

            match LocalSocketStream::connect(pipe_name) {
                Ok(_) => true,
                Err(e) => {
                    tracing::debug!("Internal daemon check failed (path={pipe_name}): {e}");
                    false
                }
            }
        }
    }

    /// Start mpv daemon with IPC socket.
    ///
    /// # Errors
    /// Returns error if spawning the daemon fails.
    pub fn start_daemon(&self, _options: Vec<String>) -> Result<()> {
        if self.is_daemon_alive_internal() {
            tracing::info!("daemon already running on {}", self.socket_path.display());
            return Ok(());
        }
        self.spawn_daemon()
    }

    /// # Errors
    /// Returns an error if the mpv process cannot be spawned or if the IPC socket
    /// does not become ready within the timeout.
    #[allow(clippy::too_many_lines)]
    fn spawn_daemon(&self) -> Result<()> {
        // Clean up socket file on Unix
        #[cfg(unix)]
        let _ = std::fs::remove_file(&self.socket_path);

        tracing::info!("starting daemon with socket {}", self.socket_path.display());

        // MPV args
        let mut command = Command::new("mpv");
        command
            .arg(format!("--input-ipc-server={}", self.socket_path.display()))
            .arg("--idle=yes")
            .arg("--force-window=no")
            .arg("--no-ytdl");

        #[cfg(windows)]
        command.arg("--ontop");

        #[cfg(unix)]
        command.arg("--vo=gpu").arg("--gpu-api=opengl");

        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        #[cfg(unix)]
        command.arg("--hwdec=no"); // Explicit disable on Linux causing issues? or user pref.

        // Windows might want generic hwdec
        #[cfg(windows)]
        command.arg("--hwdec=auto");

        tracing::info!(
            "spawn arg: --input-ipc-server={}",
            self.socket_path.display()
        );

        let mut child = command.spawn().context("Failed to spawn mpv daemon")?;

        // Take handles before moving child into mutex
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        *self.process.lock().unwrap_or_else(|e| {
            tracing::warn!("MPV process lock poisoned: {e}");
            e.into_inner()
        }) = Some(child);

        // Wait for ready
        for _ in 0..100 {
            if self.is_daemon_alive_internal() {
                tracing::info!("daemon ready, configuring key bindings");
                self.configure_bindings();
                return Ok(());
            }

            // Checks if process exited
            let mut guard = self.process.lock().unwrap_or_else(|e| {
                tracing::warn!("MPV process lock poisoned (wait loop): {e}");
                e.into_inner()
            });
            if let Some(child) = guard.as_mut()
                && let Ok(Some(status)) = child.try_wait()
            {
                // Process exited!
                let mut err_msg = String::new();
                if let Some(mut err) = stderr {
                    let _ = err.read_to_string(&mut err_msg);
                }
                if let Some(mut out) = stdout {
                    let _ = out.read_to_string(&mut err_msg);
                }
                anyhow::bail!("mpv exited early with status {status}: {err_msg}");
            }
            drop(guard);

            std::thread::sleep(Duration::from_millis(100));
        }

        anyhow::bail!("mpv socket not ready after 10 seconds")
    }

    fn configure_bindings(&self) {
        let bindings = [r#"{"command": ["keybind", "q", "stop"]}"#];

        // Refactored to avoid try_clone if possible, or gracefully fail binding config on Windows
        // For Windows, opening a new connection for each logic might be safer if cloning fails?
        // But the daemon handles multiple clients? yes.

        // Attempt complex logic only if cloning supported (Unix)
        // Or rewrite to sequential write-read without clone

        if let Ok(mut stream) = self.connect() {
            let _ = stream.set_read_timeout_custom(Some(self.connect_timeout));

            // We use a specific pattern: Write command, then Read response immediately.
            // We can do this with a single stream reference!
            // But BufferReader takes ownership?
            // We can create a new BufferReader for each read? inefficient but works.
            // Or better: manual read loop without BufReader ownership of the whole stream?
            // Or simply:

            for cmd in bindings {
                // Write
                if writeln!(stream, "{cmd}").is_ok() {
                    let _ = stream.flush();

                    // Read response
                    // We need to read one line.
                    // On Windows/Unix IPC, reading can be done byte by byte until newline
                    // or use a short-lived buffer.
                    let mut byte = [0u8; 1];
                    // Simple implementation reading until \n
                    while let Ok(n) = stream.read(&mut byte) {
                        if n == 0 {
                            break;
                        } // EOF
                        if byte[0] == b'\n' {
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Ensure mpv daemon is running.
    ///
    /// # Errors
    /// Returns error if spawning the daemon fails.
    fn ensure_daemon(&self) -> Result<()> {
        if !self.is_daemon_alive_internal() {
            tracing::info!("daemon not running, restarting");
            self.spawn_daemon()?;
        }
        Ok(())
    }

    /// Send a command to the mpv daemon.
    ///
    /// # Errors
    /// Returns error if connection or communication fails.
    pub fn send_command(&self, cmd: &str) -> Result<String> {
        let mut stream = self.connect()?;

        stream.set_write_timeout_custom(Some(self.command_timeout))?;
        stream.set_read_timeout_custom(Some(self.command_timeout))?;

        writeln!(stream, "{cmd}")?;
        stream.flush()?;

        let mut reader = BufReader::new(stream);
        let mut response = String::new();
        let _ = reader.read_line(&mut response);
        Ok(response)
    }

    /// Play a playlist of items.
    ///
    /// # Errors
    /// Returns error if communication with daemon fails.
    pub fn play_playlist(&self, items: &[PlaylistItem]) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        self.ensure_daemon()?;

        let playlist_content = generate_m3u_content(items);

        let temp_dir = std::env::temp_dir();
        let playlist_path = temp_dir.join("soromantic_playlist.m3u");
        std::fs::write(&playlist_path, playlist_content)
            .context("Failed to write playlist file")?;

        let _ = self.send_command(r#"{"command": ["set_property", "pause", false]}"#);
        let _ = self.send_command(r#"{"command": ["set_property", "fullscreen", true]}"#);

        let path_str = playlist_path.to_string_lossy();
        tracing::info!("loading playlist {path_str}");

        // Escape for JSON string
        // Windows paths have backslashes, need double escaping for JSON: \ -> \\ -> \\\\
        // Also quotes
        let escaped_path = path_str.replace('\\', "\\\\").replace('"', "\\\"");

        let cmd = format!(r#"{{"command": ["loadfile", "{escaped_path}", "replace"]}}"#);

        let _ = self.send_command(&cmd)?;
        Ok(())
    }

    /// Play a single video.
    ///
    /// # Errors
    /// Returns error if communication with daemon fails.
    pub fn play_video(&self, path: &str, title: String) -> Result<()> {
        self.play_playlist(&[crate::types::PlaylistItem {
            path: path.to_string(),
            title,
            intervals: None,
        }])
    }

    /// # Errors
    /// Returns error if connection to daemon fails.
    pub fn quit(&self) -> Result<()> {
        if let Ok(mut stream) = self.connect() {
            let _ = writeln!(stream, r#"{{"command": ["quit"]}}"#);
        }
        Ok(())
    }
}

impl Drop for MpvClient {
    fn drop(&mut self) {
        let _ = self.quit();
        #[cfg(unix)]
        let _ = std::fs::remove_file(&self.socket_path);

        let child_opt = self
            .process
            .lock()
            .unwrap_or_else(|e| {
                tracing::warn!("MPV process lock poisoned (drop): {}", e);
                e.into_inner()
            })
            .take();

        if let Some(mut child) = child_opt {
            let _ = child.wait();
        }
    }
}

fn generate_m3u_content(items: &[PlaylistItem]) -> String {
    let mut content = String::from("#EXTM3U\n");
    for item in items {
        match &item.intervals {
            Some(intervals) if !intervals.is_empty() => {
                // Generate edl:// URI for intervals
                // Syntax: edl://%length%filename,start,len;...
                let mut edl_segments = Vec::new();
                for (start, end) in intervals {
                    let length = end - start;
                    let length = if length < 0.0 { 0.0 } else { length };

                    let path_bytes = item.path.as_bytes();
                    let len = path_bytes.len();
                    let path = &item.path;
                    let segment = format!("%{len}%{path},{start},{length}");
                    edl_segments.push(segment);
                }
                let edl_uri = format!("edl://{}", edl_segments.join(";"));
                let title = &item.title;
                let _ = write!(content, "#EXTINF:-1,{title}\n{edl_uri}\n");
            }
            _ => {
                let title = &item.title;
                let path = &item.path;
                let _ = write!(content, "#EXTINF:-1,{title}\n{path}\n");
            }
        }
    }
    content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_m3u_content() {
        let items = vec![PlaylistItem {
            path: "/path/to/video1.mp4".to_string(),
            title: "Video Title 1".to_string(),
            intervals: None,
        }];
        let content = generate_m3u_content(&items);
        assert!(content.contains("#EXTM3U"));
        assert!(content.contains("Video Title 1"));
    }

    #[test]
    fn test_generate_m3u_content_intervals() {
        let items = vec![PlaylistItem {
            path: "/path/to/video1.mp4".to_string(),
            title: "Scene 1".to_string(),
            intervals: Some(vec![(10.0, 20.0), (30.0, 40.0)]),
        }];
        let content = generate_m3u_content(&items);
        assert!(content.contains("edl://"));
        // Check for length syntax: %19%/path/to/video1.mp4,10,10
        assert!(content.contains("%19%/path/to/video1.mp4,10,10"));
        assert!(content.contains(";%19%"));
    }
}
