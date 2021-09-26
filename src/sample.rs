// Structures and methods for sampled data. Kept things discrete to take advantage of match.
use std::fs::{read_dir, read_to_string};
use std::path::PathBuf;
use std::process;
use std::time::Instant;
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy)]
pub struct TaskSample {
    pub timestamp: Instant,
    pub id: u32,
    pub cpu: u32,
    pub user: u128,
    pub system: u128
}

#[derive(Debug, Clone, Copy)]
pub struct CpuSample {
    pub timestamp: Instant,
    pub cpu: u32,
    pub user: u128,
    pub nice: u128,
    pub system: u128,
    pub idle: u128,
    pub iowait: u128,
    pub irq: u128,
    pub softirq: u128,
    pub steal: u128,
    pub guest: u128,
    pub guest_nice: u128
}

#[derive(Debug, Clone, Copy)]
pub struct EnergySample {
    pub timestamp: Instant,
    pub socket: u32,
    pub core: f32,
    pub dram: f32,
    pub gpu: f32,
    pub package: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum Sample {
    Task(TaskSample),
    Cpu(CpuSample),
    Energy(EnergySample),
}

impl Sample {
    pub fn get_timestamp(&self) -> Instant {
        match self {
            Sample::Task(task) => task.timestamp,
            Sample::Cpu(cpu) => cpu.timestamp,
            Sample::Energy(energy) => energy.timestamp,
        }
    }

    // this is a bad hack
    pub(crate) fn key(&self) -> u32 {
        match self {
            Sample::Task(..) => 0,
            Sample::Cpu(..) => 1,
            Sample::Energy(..) => 2,
        }
    }
}

impl PartialEq for Sample {
    fn eq(&self, other: &Self) -> bool {
        self.get_timestamp() == other.get_timestamp()
    }
}

impl Eq for Sample {}

impl PartialOrd for Sample {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.get_timestamp().partial_cmp(&other.get_timestamp())
    }
}

impl Ord for Sample {
    fn cmp(&self, other: &Self) -> Ordering {
        self.get_timestamp().cmp(&other.get_timestamp())
    }
}

// Method to sample data from /proc/[pid]/task/[tid]/stat; refer to
// https://man7.org/linux/man-pages/man5/proc.5.html for details about /proc
static ENTRY_COUNT: usize = 52;

pub fn sample_tasks(pid: u32) -> Vec<Sample> {
    let start = Instant::now();
    read_dir(["/proc", &pid.to_string(), "task"].iter().collect::<PathBuf>())
        .unwrap()
        .map(|task| -> Sample {
            let mut stat = task.unwrap().path();
            stat.push("stat");
            let stats = read_to_string(stat).unwrap();
            let stats: Vec<&str> = stats.split(' ').collect();
            let offset = stats.len() - ENTRY_COUNT;

            Sample::Task(TaskSample {
                timestamp: start,
                id: stats[0].parse().unwrap(),
                cpu: stats[38 + offset].parse().unwrap(),
                user: stats[13 + offset].parse().unwrap(),
                system: stats[14 + offset].parse().unwrap(),
            })
        }).collect()
}

// Method to sample data from /proc/stat; refer to
// https://man7.org/linux/man-pages/man5/proc.5.html for details about /proc
pub fn sample_cpus() -> Vec<Sample> {
    let start = Instant::now();
    read_to_string(["/proc", "stat"].iter().collect::<PathBuf>())
        .unwrap()
        .split('\n')
        .skip(1) // first entry is system total
        .take_while(|stat| {stat.contains("cpu")})
        .map(|stat| -> Sample {
            let stats: Vec<&str> = stat.split(' ').collect();
            Sample::Cpu(CpuSample {
                timestamp: start,
                cpu: stats[0][3..].parse().unwrap(),
                user: stats[1].parse().unwrap(),
                nice: stats[2].parse().unwrap(),
                system: stats[3].parse().unwrap(),
                idle: stats[4].parse().unwrap(),
                iowait: stats[5].parse().unwrap(),
                irq: stats[6].parse().unwrap(),
                softirq: stats[7].parse().unwrap(),
                steal: stats[8].parse().unwrap(),
                guest: stats[9].parse().unwrap(),
                guest_nice: stats[10].parse().unwrap(),
            })
        }).collect()
}

// add to this as needed
pub(crate) static SOURCES: [fn() -> Vec<Sample>; 2] = [
    || sample_tasks(process::id()),
    || sample_cpus(),
];
