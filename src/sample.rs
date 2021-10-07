// Structures and methods for sampled data. Kept things discrete to take advantage of match.
use std::fs;
use std::io;
use std::process;
use std::time::SystemTime;

use procfs::{CpuTime, KernelStats};
use procfs::process::{Process, Stat};

pub enum Sample {
    Cpu(CpuSample),
    Task(TaskSample),
    Rapl(RaplSample)
}

#[derive(Debug)]
pub struct TaskSample {
    pub timestamp: u128,
    pub tasks: Option<Vec<Stat>>
}

impl TaskSample {
    pub fn new() -> Sample {
        TaskSample::for_pid(process::id() as i32)
    }

    pub fn for_pid(id: i32) -> Sample {
        Sample::Task(TaskSample {
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
            tasks: sample_tasks(id)
        })
    }
}

fn sample_tasks(pid: i32) -> Option<Vec<Stat>> {
    if let Ok(main) = Process::new(pid) {
        if let Ok(tasks) = main.tasks() {
            return Some(tasks.flatten().map(|t| t.stat().unwrap()).collect())
        }
    }
    None
}

#[derive(Debug)]
pub struct CpuSample {
    pub timestamp: u128,
    pub cpus: Option<Vec<CpuTime>>
}

impl CpuSample {
    pub fn new() -> Sample {
        Sample::Cpu(CpuSample {
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
            cpus: sample_cpus(),
        })
    }
}

fn sample_cpus() -> Option<Vec<CpuTime>> {
    match KernelStats::new() {
        Ok(stats) => Some(stats.cpu_time),
        _ => None
    }
}

#[derive(Debug)]
pub struct RaplReading {
    pub socket: u32,
    pub dram: u64,
    pub pkg: u64
}

#[derive(Debug)]
pub struct RaplSample {
    pub timestamp: u128,
    pub readings: Option<Vec<RaplReading>>
}

impl RaplSample {
    pub fn new() -> Sample {
        Sample::Rapl(RaplSample {
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
            readings: sample_rapl(),
        })
    }
}

fn sample_rapl() -> Option<Vec<RaplReading>> {
    Some((0..2).map(sample_socket).filter_map(Result::ok).collect())
}

fn sample_socket(socket: u32) -> Result<RaplReading, io::Error> {
    Ok(RaplReading {
        socket,
        dram: parse_rapl_energy(format!("/sys/class/powercap/intel-rapl:{}:0/energy_uj", socket)),
        pkg: parse_rapl_energy(format!("/sys/class/powercap/intel-rapl:{}/energy_uj", socket))
    })
}

fn parse_rapl_energy(rapl_energy_file: String) -> u64 {
    fs::read_to_string(rapl_energy_file).unwrap().trim().parse().unwrap()
}
