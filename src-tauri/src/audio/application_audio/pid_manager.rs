use anyhow::Result;
use colored::Colorize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use sysinfo::{System, Process, Pid};
use tracing::info;

const MAX_CHANNELS: usize = 16;

#[derive(Debug, Clone)]
pub struct ApplicationInfo {
    pub pid: i32,
    pub name: String,
    pub channel: i32,
}

pub struct PIDManager {
    system: Arc<Mutex<System>>,
    pid_to_channel: Arc<Mutex<HashMap<i32, i32>>>,
    channel_to_pid: Arc<Mutex<HashMap<i32, i32>>>,
    next_channel: Arc<Mutex<i32>>,
}

impl PIDManager {
    pub fn new() -> Self {
        Self {
            system: Arc::new(Mutex::new(System::new_all())),
            pid_to_channel: Arc::new(Mutex::new(HashMap::new())),
            channel_to_pid: Arc::new(Mutex::new(HashMap::new())),
            next_channel: Arc::new(Mutex::new(0)),
        }
    }

    pub fn list_applications(&self) -> Result<Vec<ApplicationInfo>> {
        let mut system = self.system.lock().unwrap();
        system.refresh_processes_specifics(sysinfo::ProcessRefreshKind::nothing());

        let mut apps = Vec::new();

        for (pid, process) in system.processes() {
            let process_name = process.name().to_str().unwrap_or("");

            if process_name.is_empty() {
                continue;
            }

            let pid_i32 = pid.as_u32() as i32;
            let channel = self.get_channel_for_pid(pid_i32);

            apps.push(ApplicationInfo {
                pid: pid_i32,
                name: process_name.to_string(),
                channel: channel.unwrap_or(-1),
            });
        }

        apps.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(apps)
    }

    pub fn get_process_name(&self, pid: i32) -> Result<String> {
        let mut system = self.system.lock().unwrap();
        system.refresh_processes_specifics(sysinfo::ProcessRefreshKind::nothing());

        let sysinfo_pid = Pid::from_u32(pid as u32);

        if let Some(process) = system.process(sysinfo_pid) {
            Ok(process.name().to_str().unwrap_or("Unknown").to_string())
        } else {
            anyhow::bail!("Process with PID {} not found", pid)
        }
    }

    pub fn allocate_channel(&self, pid: i32) -> Result<i32> {
        let mut pid_to_channel = self.pid_to_channel.lock().unwrap();
        let mut channel_to_pid = self.channel_to_pid.lock().unwrap();

        if let Some(&existing_channel) = pid_to_channel.get(&pid) {
            info!(
                "{} PID {} already has channel {}",
                "CHANNEL_EXISTS".yellow(),
                pid,
                existing_channel
            );
            return Ok(existing_channel);
        }

        let mut next_channel = self.next_channel.lock().unwrap();

        while *next_channel < MAX_CHANNELS as i32 {
            let channel = *next_channel;
            *next_channel += 1;

            if !channel_to_pid.contains_key(&channel) {
                pid_to_channel.insert(pid, channel);
                channel_to_pid.insert(channel, pid);

                info!(
                    "{} PID {} -> channel {}",
                    "CHANNEL_ALLOCATED".green(),
                    pid,
                    channel
                );

                return Ok(channel);
            }
        }

        anyhow::bail!("No available channels (max {})", MAX_CHANNELS)
    }

    pub fn free_channel(&self, pid: i32) -> Result<()> {
        let mut pid_to_channel = self.pid_to_channel.lock().unwrap();
        let mut channel_to_pid = self.channel_to_pid.lock().unwrap();

        if let Some(channel) = pid_to_channel.remove(&pid) {
            channel_to_pid.remove(&channel);
            info!("{} PID {} (channel {})", "CHANNEL_FREED".green(), pid, channel);
            Ok(())
        } else {
            anyhow::bail!("PID {} has no allocated channel", pid)
        }
    }

    pub fn get_channel_for_pid(&self, pid: i32) -> Option<i32> {
        let pid_to_channel = self.pid_to_channel.lock().unwrap();
        pid_to_channel.get(&pid).copied()
    }

    pub fn get_pid_for_channel(&self, channel: i32) -> Option<i32> {
        let channel_to_pid = self.channel_to_pid.lock().unwrap();
        channel_to_pid.get(&channel).copied()
    }

    pub fn get_all_mappings(&self) -> HashMap<i32, i32> {
        let pid_to_channel = self.pid_to_channel.lock().unwrap();
        pid_to_channel.clone()
    }

    pub fn clear_all(&self) {
        let mut pid_to_channel = self.pid_to_channel.lock().unwrap();
        let mut channel_to_pid = self.channel_to_pid.lock().unwrap();
        let mut next_channel = self.next_channel.lock().unwrap();

        pid_to_channel.clear();
        channel_to_pid.clear();
        *next_channel = 0;

        info!("{}", "ALL_CHANNELS_CLEARED".yellow());
    }
}

impl Default for PIDManager {
    fn default() -> Self {
        Self::new()
    }
}
