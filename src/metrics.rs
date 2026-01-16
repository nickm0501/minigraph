use std::sync::{atomic::AtomicU64, atomic::Ordering, Arc};

use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Metrics {
    actor_cmd_drops_total: AtomicU64,
    fanout_drops_total: AtomicU64,

    // Last sampled values.
    server_cpu_usage_x100: AtomicU64,
    // sysinfo reports memory in KiB; expose bytes.
    server_memory_bytes: AtomicU64,

    // Aggregates across samples (best-effort, saturating).
    server_samples_total: AtomicU64,
    server_cpu_usage_x100_sum: AtomicU64,
    server_cpu_usage_x100_max: AtomicU64,
    server_memory_bytes_sum: AtomicU64,
    server_memory_bytes_max: AtomicU64,
}

impl Metrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn inc_actor_cmd_drop(&self) {
        self.actor_cmd_drops_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_fanout_drop(&self) {
        self.fanout_drops_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_resource_sample(&self, cpu_usage_x100: u64, memory_bytes: u64) {
        self.server_cpu_usage_x100
            .store(cpu_usage_x100, Ordering::Relaxed);
        self.server_memory_bytes
            .store(memory_bytes, Ordering::Relaxed);

        self.server_samples_total.fetch_add(1, Ordering::Relaxed);

        saturating_fetch_add(&self.server_cpu_usage_x100_sum, cpu_usage_x100);
        self.server_cpu_usage_x100_max
            .fetch_max(cpu_usage_x100, Ordering::Relaxed);

        saturating_fetch_add(&self.server_memory_bytes_sum, memory_bytes);
        self.server_memory_bytes_max
            .fetch_max(memory_bytes, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let samples = self.server_samples_total.load(Ordering::Relaxed);
        let cpu_sum = self.server_cpu_usage_x100_sum.load(Ordering::Relaxed);
        let mem_sum = self.server_memory_bytes_sum.load(Ordering::Relaxed);

        let cpu_avg = if samples == 0 { 0 } else { cpu_sum / samples };
        let mem_avg = if samples == 0 { 0 } else { mem_sum / samples };

        MetricsSnapshot {
            actor_cmd_drops_total: self.actor_cmd_drops_total.load(Ordering::Relaxed),
            fanout_drops_total: self.fanout_drops_total.load(Ordering::Relaxed),

            server_samples_total: samples,

            server_cpu_usage_x100: self.server_cpu_usage_x100.load(Ordering::Relaxed),
            server_cpu_usage_x100_avg: cpu_avg,
            server_cpu_usage_x100_max: self.server_cpu_usage_x100_max.load(Ordering::Relaxed),

            server_memory_bytes: self.server_memory_bytes.load(Ordering::Relaxed),
            server_memory_bytes_avg: mem_avg,
            server_memory_bytes_max: self.server_memory_bytes_max.load(Ordering::Relaxed),
        }
    }
}

fn saturating_fetch_add(target: &AtomicU64, value: u64) {
    let mut current = target.load(Ordering::Relaxed);
    loop {
        let next = current.saturating_add(value);
        match target.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return,
            Err(updated) => current = updated,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub actor_cmd_drops_total: u64,
    pub fanout_drops_total: u64,

    pub server_samples_total: u64,

    // CPU is expressed in "percent * 100" (so 5449 == 54.49%).
    pub server_cpu_usage_x100: u64,
    pub server_cpu_usage_x100_avg: u64,
    pub server_cpu_usage_x100_max: u64,

    pub server_memory_bytes: u64,
    pub server_memory_bytes_avg: u64,
    pub server_memory_bytes_max: u64,
}

pub async fn run_resource_sampler(metrics: Arc<Metrics>) {
    use sysinfo::{Pid, System};

    let pid = Pid::from_u32(std::process::id());
    let mut system = System::new();

    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        interval.tick().await;

        system.refresh_process(pid);
        if let Some(process) = system.process(pid) {
            let cpu_usage_x100 = (process.cpu_usage() * 100.0) as u64;
            let memory_bytes = process.memory().saturating_mul(1024);

            metrics.record_resource_sample(cpu_usage_x100, memory_bytes);
        }
    }
}
