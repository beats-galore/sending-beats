use anyhow::{Context, Result};
use colored::Colorize;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;
use tracing::info;

const SOCKET_PATH: &str = "/tmp/sendin_beats_helper.sock";
const SOCKET_TIMEOUT: Duration = Duration::from_secs(5);

const CMD_MAP_PID: u8 = 0x01;
const CMD_UNMAP_PID: u8 = 0x02;

pub struct IPCClient {
    socket_path: String,
}

impl IPCClient {
    pub fn new() -> Self {
        Self {
            socket_path: SOCKET_PATH.to_string(),
        }
    }

    pub fn with_socket_path(socket_path: String) -> Self {
        Self { socket_path }
    }

    fn connect(&self) -> Result<UnixStream> {
        let mut stream = UnixStream::connect(&self.socket_path)
            .context(format!("Failed to connect to helper at {}", self.socket_path))?;

        stream
            .set_read_timeout(Some(SOCKET_TIMEOUT))
            .context("Failed to set read timeout")?;

        stream
            .set_write_timeout(Some(SOCKET_TIMEOUT))
            .context("Failed to set write timeout")?;

        Ok(stream)
    }

    pub fn map_pid_to_channel(&self, pid: i32, channel: i32) -> Result<()> {
        let mut stream = self.connect()?;

        let mut message = [0u8; 9];
        message[0] = CMD_MAP_PID;
        message[1] = ((pid >> 24) & 0xFF) as u8;
        message[2] = ((pid >> 16) & 0xFF) as u8;
        message[3] = ((pid >> 8) & 0xFF) as u8;
        message[4] = (pid & 0xFF) as u8;
        message[5] = ((channel >> 24) & 0xFF) as u8;
        message[6] = ((channel >> 16) & 0xFF) as u8;
        message[7] = ((channel >> 8) & 0xFF) as u8;
        message[8] = (channel & 0xFF) as u8;

        stream
            .write_all(&message)
            .context("Failed to write map command")?;

        let mut response = [0u8; 1];
        stream
            .read_exact(&mut response)
            .context("Failed to read response")?;

        if response[0] != 0x00 {
            anyhow::bail!("Helper returned error: 0x{:02X}", response[0]);
        }

        info!("{} PID {} -> channel {}", "IPC_MAP_SUCCESS".green(), pid, channel);

        Ok(())
    }

    pub fn unmap_pid(&self, pid: i32) -> Result<()> {
        let mut stream = self.connect()?;

        let mut message = [0u8; 9];
        message[0] = CMD_UNMAP_PID;
        message[1] = ((pid >> 24) & 0xFF) as u8;
        message[2] = ((pid >> 16) & 0xFF) as u8;
        message[3] = ((pid >> 8) & 0xFF) as u8;
        message[4] = (pid & 0xFF) as u8;
        message[5] = 0;
        message[6] = 0;
        message[7] = 0;
        message[8] = 0;

        stream
            .write_all(&message)
            .context("Failed to write unmap command")?;

        let mut response = [0u8; 1];
        stream
            .read_exact(&mut response)
            .context("Failed to read response")?;

        if response[0] != 0x00 {
            anyhow::bail!("Helper returned error: 0x{:02X}", response[0]);
        }

        info!("{} PID {}", "IPC_UNMAP_SUCCESS".green(), pid);

        Ok(())
    }

    pub fn is_helper_running(&self) -> bool {
        UnixStream::connect(&self.socket_path).is_ok()
    }
}

impl Default for IPCClient {
    fn default() -> Self {
        Self::new()
    }
}
