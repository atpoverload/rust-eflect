use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use crate::protos::data_set::EflectDataSet;
use crate::sample::{Sample, sample_cpus, sample_rapl, sample_tasks};

static DEFAULT_PERIOD_MS: u64 = 50;

pub struct Sampler {
    pid: i32,
    period: Duration,
    is_running: Arc<AtomicBool>,
    sender: Sender<Sample>,
    receiver: Receiver<Sample>,
}

impl Sampler {
    pub fn new() -> Sampler {
        Sampler::with_period(DEFAULT_PERIOD_MS)
    }

    pub fn with_period(period: u64) -> Sampler {
        Sampler::for_process_with_period(process::id() as i32, period)
    }

    pub fn for_process(pid: i32) -> Sampler {
        Sampler::for_process_with_period(pid, DEFAULT_PERIOD_MS)
    }

    pub fn for_process_with_period(pid: i32, period: u64) -> Sampler {
        let (sender, receiver) = channel::<Sample>();
        Sampler {
            pid,
            period: Duration::from_millis(period),
            is_running: Arc::new(AtomicBool::new(false)),
            sender,
            receiver,
        }
    }

    pub fn start(&mut self) {
        self.is_running.store(true, Ordering::Relaxed);
        let pid = self.pid;
        self.collect_from(sample_cpus);
        self.collect_from(sample_rapl);
        self.collect_from(move || sample_tasks(pid));
    }

    pub fn stop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
    }

    pub fn read(&mut self) -> EflectDataSet {
        let mut data_set = EflectDataSet::new();
        while let Ok(sample) = self.receiver.try_recv() {
            match sample {
                Sample::Cpu(sample) => data_set.cpu.push(sample),
                Sample::Task(sample) => data_set.task.push(sample),
                Sample::Rapl(sample) => data_set.rapl.push(sample)
            }
        }
        data_set
    }

    fn collect_from<F>(&self, mut source: F)
        where F: FnMut() -> Sample + Send + Sync + Clone + 'static {
        let is_running = self.is_running.clone();
        let sender = self.sender.clone();
        let period = self.period;

        thread::spawn(move || {
            while is_running.load(Ordering::Relaxed) {
                let start = Instant::now();
                sender.send(source()).unwrap();
                thread::sleep(period - (Instant::now() - start));
            }
        });
    }
}

impl Default for Sampler {
    fn default() -> Sampler {
        Sampler::new()
    }
}
