// Queue state tracking for SPMC queues that don't expose occupancy data
use colored::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn};

/// Commands for updating queue state from different threads
#[derive(Debug, Clone)]
pub enum QueueCommand {
    /// Producer wrote samples to queue
    SamplesWritten { queue_id: String, count: usize },
    /// Consumer read samples from queue
    SamplesRead { queue_id: String, count: usize },
    /// Register a new queue with capacity
    RegisterQueue { queue_id: String, capacity: usize },
    /// Remove queue tracking
    UnregisterQueue { queue_id: String },
}

/// Queue state information
#[derive(Debug, Clone)]
pub struct QueueInfo {
    pub queue_id: String,
    pub capacity: usize,
    pub estimated_occupancy: usize,
    pub total_written: usize,
    pub total_read: usize,
    pub usage_percent: f32,
    pub available: usize,
}

impl QueueInfo {
    pub fn new(queue_id: String, capacity: usize) -> Self {
        Self {
            queue_id,
            capacity,
            estimated_occupancy: 0,
            total_written: 0,
            total_read: 0,
            usage_percent: 0.0,
            available: capacity,
        }
    }

    /// Update with new write operation
    fn on_samples_written(&mut self, count: usize) {
        self.total_written += count;
        self.update_derived_fields();
    }

    /// Update with new read operation
    fn on_samples_read(&mut self, count: usize) {
        self.total_read += count;
        self.update_derived_fields();
    }

    /// Calculate derived fields from write/read counters
    fn update_derived_fields(&mut self) {
        // Estimate occupancy as difference between written and read
        // This can temporarily go negative if reads are reported before writes
        let occupancy_signed = self.total_written as i64 - self.total_read as i64;
        self.estimated_occupancy = occupancy_signed.max(0) as usize;

        // Clamp to capacity (queue can't hold more than capacity)
        self.estimated_occupancy = self.estimated_occupancy.min(self.capacity);

        // Calculate derived metrics
        self.usage_percent = (self.estimated_occupancy as f32 / self.capacity as f32) * 100.0;
        self.available = self.capacity.saturating_sub(self.estimated_occupancy);
    }
}

/// Manages queue state tracking for multiple SPMC queues
pub struct QueueManager {
    /// Queue state by queue ID
    queues: HashMap<String, QueueInfo>,

    /// Command receiver for queue updates
    command_rx: mpsc::UnboundedReceiver<QueueCommand>,

    /// Command sender (cloned and distributed to threads)
    command_tx: mpsc::UnboundedSender<QueueCommand>,
}

impl QueueManager {
    pub fn new() -> Self {
        let (command_tx, command_rx) = mpsc::unbounded_channel();

        Self {
            queues: HashMap::new(),
            command_rx,
            command_tx,
        }
    }

    /// Get a command sender for external threads
    pub fn get_command_sender(&self) -> mpsc::UnboundedSender<QueueCommand> {
        self.command_tx.clone()
    }

    /// Get current state of a queue
    pub fn get_queue_info(&self, queue_id: &str) -> Option<QueueInfo> {
        self.queues.get(queue_id).cloned()
    }

    /// Get state of all queues
    pub fn get_all_queue_info(&self) -> Vec<QueueInfo> {
        self.queues.values().cloned().collect()
    }

    /// Run the queue manager (processes commands)
    pub async fn run(&mut self) {
        info!("🎯 {}: Queue manager starting", "QUEUE_MANAGER".green());

        while let Some(command) = self.command_rx.recv().await {
            self.handle_command(command).await;
        }

        info!("🎯 {}: Queue manager stopped", "QUEUE_MANAGER".green());
    }

    async fn handle_command(&mut self, command: QueueCommand) {
        match command {
            QueueCommand::RegisterQueue { queue_id, capacity } => {
                info!(
                    "🎯 {}: Registering queue {} with capacity {}",
                    "QUEUE_REGISTER".green(),
                    queue_id,
                    capacity
                );
                self.queues
                    .insert(queue_id.clone(), QueueInfo::new(queue_id, capacity));
            }

            QueueCommand::UnregisterQueue { queue_id } => {
                info!(
                    "🎯 {}: Unregistering queue {}",
                    "QUEUE_UNREGISTER".green(),
                    queue_id
                );
                self.queues.remove(&queue_id);
            }

            QueueCommand::SamplesWritten { queue_id, count } => {
                if let Some(queue_info) = self.queues.get_mut(&queue_id) {
                    queue_info.on_samples_written(count);
                } else {
                    warn!(
                        "🎯 {}: Unknown queue {} for samples written",
                        "QUEUE_WARNING".yellow(),
                        queue_id
                    );
                }
            }

            QueueCommand::SamplesRead { queue_id, count } => {
                if let Some(queue_info) = self.queues.get_mut(&queue_id) {
                    queue_info.on_samples_read(count);
                } else {
                    warn!(
                        "🎯 {}: Unknown queue {} for samples read",
                        "QUEUE_WARNING".yellow(),
                        queue_id
                    );
                }
            }
        }
    }
}

/// Thread-safe queue state tracker using atomic counters
/// Alternative approach for real-time contexts that can't use async commands
#[derive(Clone)]
pub struct AtomicQueueTracker {
    pub queue_id: String,
    pub capacity: usize,
    pub current_occupancy: Arc<AtomicUsize>,
    pub total_written: Arc<AtomicUsize>, // Keep for diagnostics
    pub total_read: Arc<AtomicUsize>,    // Keep for diagnostics
}

impl AtomicQueueTracker {
    pub fn new(queue_id: String, capacity: usize) -> Self {
        Self {
            queue_id,
            capacity,
            current_occupancy: Arc::new(AtomicUsize::new(0)),
            total_written: Arc::new(AtomicUsize::new(0)),
            total_read: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Record samples written (called from producer thread) - ADD to queue occupancy
    pub fn record_samples_written(&self, count: usize) {
        let occupancy_before_add = self.current_occupancy.load(Ordering::Relaxed);
        self.current_occupancy.fetch_add(count, Ordering::Relaxed);
        self.total_written.fetch_add(count, Ordering::Relaxed); // Keep for diagnostics
        let current_occupancy = self.current_occupancy.load(Ordering::Relaxed);
        println!(
            "QUEUE_MANAGER_SAMPLES: wrote {} samples, current occupancy: {}, prior: {}",
            count,
            current_occupancy,
            occupancy_before_add
        );
    }

    /// Record samples read (called from consumer thread) - SUBTRACT from queue occupancy
    pub fn record_samples_read(&self, count: usize) {
        let occupancy_before_add =  self.current_occupancy.load(Ordering::Relaxed);
        self.current_occupancy.fetch_sub(count, Ordering::Relaxed);
        self.total_read.fetch_add(count, Ordering::Relaxed); // Keep for diagnostics
        let current_occupancy = self.current_occupancy.load(Ordering::Relaxed);
        println!(
            "QUEUE_MANAGER_SAMPLES: read {} samples, current occupancy: {}, prior: {}",
            count,
            current_occupancy,
            occupancy_before_add
        );
    }

    /// Get current queue info (can be called from any thread)
    pub fn get_queue_info(&self) -> QueueInfo {
        let total_written = self.total_written.load(Ordering::Relaxed);
        let total_read = self.total_read.load(Ordering::Relaxed);
        let current_occupancy = self.current_occupancy.load(Ordering::Relaxed);

        // Clamp occupancy to capacity (can't exceed queue size)
        let estimated_occupancy = current_occupancy.min(self.capacity);

        let usage_percent = (estimated_occupancy as f32 / self.capacity as f32) * 100.0;
        let available = self.capacity.saturating_sub(estimated_occupancy);

        println!(
            "CURRENT_OCCUPANCY: {}, usage: {}, available: {}",
            current_occupancy, usage_percent, available
        );

        QueueInfo {
            queue_id: self.queue_id.clone(),
            capacity: self.capacity,
            estimated_occupancy,
            total_written,
            total_read,
            usage_percent,
            available,
        }
    }

}
