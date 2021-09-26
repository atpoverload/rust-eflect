use crate::sample::{Sample, SOURCES};

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;
use std::time::{Duration, Instant};

static DEFAULT_PERIOD_MS: u64 = 50;
static DEFAULT_PERIOD: Duration = Duration::from_millis(DEFAULT_PERIOD_MS);

pub struct Eflect {
    period: Duration,
    is_running: Arc<AtomicBool>,
    sender: Sender<Sample>,
    receiver: Receiver<Sample>,
}

impl Eflect {
    pub fn new() -> Eflect {
        let (sender, receiver) = channel::<Sample>();
        Eflect {
            period: DEFAULT_PERIOD,
            is_running: Arc::new(AtomicBool::new(false)),
            sender,
            receiver,
        }
    }

    pub fn with_period(period: Duration) -> Eflect {
        let (sender, receiver) = channel::<Sample>();
        Eflect {
            period,
            is_running: Arc::new(AtomicBool::new(false)),
            sender,
            receiver,
        }
    }

    pub fn with_period_ms(period: u64) -> Eflect {
        let (sender, receiver) = channel::<Sample>();
        Eflect {
            period: Duration::from_millis(period),
            is_running: Arc::new(AtomicBool::new(false)),
            sender,
            receiver,
        }
    }

    pub fn start(&mut self) {
        self.is_running.store(true, Ordering::Relaxed);

        for source in SOURCES {
            let is_running = self.is_running.clone();
            let sender = self.sender.clone();
            let period = self.period;

            thread::spawn(move || {
                while is_running.load(Ordering::Relaxed) {
                    let start = Instant::now();
                    source()
                        .iter()
                        .for_each(|&sample| { sender.send(sample).unwrap(); });
                    thread::sleep(period - (Instant::now() - start));
                }
            });
        }
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
}

impl Default for Eflect {
    fn default() -> Eflect {
        Eflect::new()
    }
}
