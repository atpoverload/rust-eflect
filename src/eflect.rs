use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use crate::sample::{Sample, CpuSample, TaskSample, RaplSample};

static DEFAULT_PERIOD_MS: u64 = 50;

pub struct Eflect {
    pid: i32,
    period: Duration,
    is_running: Arc<AtomicBool>,
    sender: Sender<Sample>,
    receiver: Receiver<Sample>,
}

impl Eflect {
    pub fn new() -> Eflect {
        Eflect::with_period(DEFAULT_PERIOD_MS)
    }

    pub fn with_period(period: u64) -> Eflect {
        Eflect::for_process_with_period(process::id() as i32, period)
    }

    pub fn for_process(pid: i32) -> Eflect {
        Eflect::for_process_with_period(pid, DEFAULT_PERIOD_MS)
    }

    pub fn for_process_with_period(pid: i32, period: u64) -> Eflect {
        let (sender, receiver) = channel::<Sample>();
        Eflect {
            pid,
            period: Duration::from_millis(period),
            is_running: Arc::new(AtomicBool::new(false)),
            sender,
            receiver,
        }
    }

    pub fn start(&mut self) {
        self.is_running.store(true, Ordering::Relaxed);
        let pid = self.pid.clone();
        self.collect_from(move || TaskSample::for_pid(pid));
        self.collect_from(|| CpuSample::new());
        self.collect_from(|| RaplSample::new());
    }

    pub fn stop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
    }

    pub fn read(&mut self) -> Vec<Sample> {
        let mut data: Vec<Sample> = Vec::new();
        while let Ok(sample) = self.receiver.try_recv() {
            data.push(sample);
        }
        data
    }

    fn collect_from<F>(&self, mut source: F)
        where F: FnMut() -> Sample + Send + Sync + Clone + 'static {
        let is_running = self.is_running.clone();
        let sender = self.sender.clone();
        let period = self.period;

        thread::spawn(move || {
            while is_running.load(Ordering::Relaxed) {
                let start = Instant::now();
                // if let sample = source() {
                sender.send(source()).unwrap();
                // }
                thread::sleep(period - (Instant::now() - start));
            }
        });
    }
}

impl Default for Eflect {
    fn default() -> Eflect {
        Eflect::new()
    }
}
