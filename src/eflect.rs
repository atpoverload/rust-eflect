use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use crate::protos::sample::{DataSet, Sample, Sample_oneof_data};
use crate::sample::{SamplingError, sample_cpus, sample_rapl, sample_tasks};

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
        self.start_sampling_from(sample_cpus);
        self.start_sampling_from(sample_rapl);
        self.start_sampling_from(move || sample_tasks(pid));
    }

    pub fn stop(&mut self) {
        self.is_running.store(false, Ordering::Relaxed);
    }

    pub fn read(&mut self) -> DataSet {
        let mut data_set = DataSet::new();
        while let Ok(sample) = self.receiver.try_recv() {
            match sample.data {
                Some(Sample_oneof_data::cpu(sample)) => data_set.cpu.push(sample),
                Some(Sample_oneof_data::task(sample)) => data_set.task.push(sample),
                Some(Sample_oneof_data::rapl(sample)) => data_set.rapl.push(sample),
                _ => println!("no sample found!")
            }
        }
        data_set
    }

    fn start_sampling_from<F>(&self, mut source: F)
        where F: FnMut() -> Result<Sample, SamplingError> + Send + Sync + Clone + 'static {
        let is_running = self.is_running.clone();
        let sender = self.sender.clone();
        let period = self.period;

        thread::spawn(move || {
            while is_running.load(Ordering::Relaxed) {
                let start = Instant::now();
                // TODO(timur): make the logging configurable?
                // match source() {
                //     Ok(sample) => sender.send(sample).unwrap(),
                //     Err(error) => println!("{}", error.message)
                // };
                if let Ok(sample) = source() {
                    sender.send(sample).unwrap();
                }
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
