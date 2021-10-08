// Structures and methods for sampled data. Kept things discrete to take advantage of match.
use std::fs;
use std::io;
use std::time::SystemTime;

use procfs::{CpuTime, KernelStats};
use procfs::process::{Process, Stat};

use crate::protos::jiffies::{CpuSample, CpuStat, TaskSample, TaskStat};
use crate::protos::rapl::{RaplReading, RaplSample};

// outer sample struct so we can do type operations
pub enum Sample {
    Cpu(CpuSample),
    Task(TaskSample),
    Rapl(RaplSample)
}

impl Sample {
    pub fn get_timestamp(&self) -> u64 {
        match self {
            Sample::Cpu(sample) => sample.get_timestamp(),
            Sample::Task(sample) => sample.get_timestamp(),
            Sample::Rapl(sample) => sample.get_timestamp(),
        }
    }
}

// code to sample /proc/stat
pub fn sample_cpus() -> Sample {
    let mut sample = CpuSample::new();
    sample.set_timestamp(SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64);
    if let Some(cpus) = read_cpus() {
        cpus.into_iter().for_each(|stat| sample.stats.push(stat));
    };
    Sample::Cpu(sample)
}


fn read_cpus() -> Option<Vec<CpuStat>> {
    match KernelStats::new() {
        Ok(stats) => Some(stats.cpu_time
            .into_iter()
            .enumerate()
            .map(|(cpu, stat)| cpu_stat_to_proto(cpu as u32, stat))
            .collect()
        ),
        _ => None
    }
}

fn cpu_stat_to_proto(cpu: u32, stat: CpuTime) -> CpuStat {
    let mut stat_proto = CpuStat::new();
    stat_proto.set_cpu(cpu);
    stat_proto.set_user(stat.user as u32);
    stat_proto.set_nice(stat.nice as u32);
    stat_proto.set_system(stat.system as u32);
    stat_proto.set_idle(stat.idle as u32);
    if let Some(jiffies) = stat.iowait {
        stat_proto.set_iowait(jiffies as u32);
    };
    if let Some(jiffies) = stat.irq {
        stat_proto.set_irq(jiffies as u32);
    };
    if let Some(jiffies) = stat.softirq {
        stat_proto.set_softirq(jiffies as u32);
    };
    if let Some(jiffies) = stat.steal {
        stat_proto.set_steal(jiffies as u32);
    };
    if let Some(jiffies) = stat.guest {
        stat_proto.set_guest(jiffies as u32);
    };
    if let Some(jiffies) = stat.guest_nice {
        stat_proto.set_guest_nice(jiffies as u32);
    };
    stat_proto
}

// code to sample /proc/[pid]/task/[tid]/stat
pub fn sample_tasks(pid: i32) -> Sample {
    let mut sample = TaskSample::new();
    sample.set_timestamp(SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64);
    if let Some(tasks) = read_tasks(pid) {
        tasks.into_iter().for_each(|s| sample.stats.push(s));
    };
    Sample::Task(sample)
}

fn read_tasks(pid: i32) -> Option<Vec<TaskStat>> {
    if let Ok(main) = Process::new(pid) {
        if let Ok(tasks) = main.tasks() {
            return Some(tasks.flatten().map(|stat| task_stat_to_proto(stat.stat().unwrap())).collect())
        }
    }
    None
}

fn task_stat_to_proto(stat: Stat) -> TaskStat {
    let mut stat_proto = TaskStat::new();
    stat_proto.set_thread_id(stat.pid as u32);
    if let Some(cpu) = stat.processor {
        stat_proto.set_cpu(cpu as u32);
        stat_proto.set_user(stat.cutime as u32);
        stat_proto.set_system(stat.cstime as u32);
    };
    stat_proto
}

// code to sample /sys/class/powercap
pub fn sample_rapl() -> Sample {
    let mut sample = RaplSample::new();
    sample.set_timestamp(SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64);
    if let Some(reading) = read_rapl() {
        reading.into_iter().for_each(|reading| sample.readings.push(reading));
    };
    Sample::Rapl(sample)
}

fn read_rapl() -> Option<Vec<RaplReading>> {
    // TODO(timur): get a generic mapping for any system instead of just vaporeon
    Some((0..2).map(read_socket).filter_map(Result::ok).collect())
}

fn read_socket(socket: u32) -> Result<RaplReading, io::Error> {
    let mut reading = RaplReading::new();
    reading.set_socket(socket);
    reading.set_dram(parse_rapl_energy(format!("/sys/class/powercap/intel-rapl:{}:0/energy_uj", socket)));
    reading.set_package(parse_rapl_energy(format!("/sys/class/powercap/intel-rapl:{}/energy_uj", socket)));
    Ok(reading)
}

fn parse_rapl_energy(rapl_energy_file: String) -> u64 {
    fs::read_to_string(rapl_energy_file).unwrap().trim().parse().unwrap()
}
