// Structures and methods for sampled data.
use std::fs;
use std::io;
use std::time::SystemTime;

use procfs::{CpuTime, KernelStats, ProcError};
use procfs::process::{Process, Stat};

use crate::protos::jiffies::{CpuSample, CpuStat, TaskSample, TaskStat};
use crate::protos::rapl::{RaplReading, RaplSample};

// enum wrapper around the proto
pub enum Sample {
    Cpu(CpuSample),
    Task(TaskSample),
    Rapl(RaplSample)
}

// TODO(timur): make real error handling
pub struct SamplingError {
    pub message: String
}

// code to sample /proc/stat
pub fn sample_cpus() -> Result<Sample, SamplingError> {
    match read_cpus() {
        Ok(stats) => {
            let mut sample = CpuSample::new();
            sample.set_timestamp(SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64);
            stats.into_iter().for_each(|stat| sample.stats.push(stat));
            Ok(Sample::Cpu(sample))
        },
        _ => Err(SamplingError{message: "there was an error with proc".to_string()})
    }
}

fn read_cpus() -> Result<Vec<CpuStat>, ProcError> {
    Ok(KernelStats::new()?.cpu_time
        .into_iter()
        .enumerate()
        .map(|(cpu, stat)| cpu_stat_to_proto(cpu as u32, stat))
        .collect())
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
pub fn sample_tasks(pid: i32) -> Result<Sample, SamplingError> {
    if let Ok(tasks) = read_tasks(pid) {
        let mut sample = TaskSample::new();
        sample.set_timestamp(SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64);
        tasks.into_iter().for_each(|s| sample.stats.push(s));
        Ok(Sample::Task(sample))
    } else {
        Err(SamplingError{message: "there was an error with proc".to_string()})
    }
}

fn read_tasks(pid: i32) -> Result<Vec<TaskStat>, ProcError> {
    Ok(Process::new(pid)?.tasks()?
        .flatten()
        .map(|stat| task_stat_to_proto(stat.stat().unwrap()))
        .collect())
}

fn task_stat_to_proto(stat: Stat) -> TaskStat {
    let mut stat_proto = TaskStat::new();
    stat_proto.set_task_id(stat.pid as u32);
    if let Some(cpu) = stat.processor {
        stat_proto.set_cpu(cpu as u32);
        stat_proto.set_user(stat.cutime as u32);
        stat_proto.set_system(stat.cstime as u32);
    };
    stat_proto
}

// code to sample /sys/class/powercap
pub fn sample_rapl() -> Result<Sample, SamplingError> {
    if let Ok(reading) = read_rapl() {
        let mut sample = RaplSample::new();
        sample.set_timestamp(SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64);
        reading.into_iter().for_each(|reading| sample.readings.push(reading));
        Ok(Sample::Rapl(sample))
    } else {
        Err(SamplingError{message: "there was an error reading /sys/class/powercap".to_string()})
    }
}

// TODO(timur): implement handling for N domains
fn read_rapl() -> Result<Vec<RaplReading>, SamplingError> {
    let readings: Vec<RaplReading> = (0..2)
        .map(read_socket)
        .filter_map(Result::ok)
        .collect();
    if !readings.is_empty() {
        Ok(readings)
    } else {
        Err(SamplingError{message: "there was an error reading /sys/class/powercap".to_string()})
    }
}

fn read_socket(socket: u32) -> Result<RaplReading, io::Error> {
    let mut reading = RaplReading::new();
    reading.set_socket(socket);
    reading.set_package(parse_rapl_energy(format!("/sys/class/powercap/intel-rapl:{}/energy_uj", socket))?);
    reading.set_dram(parse_rapl_energy(format!("/sys/class/powercap/intel-rapl:{}:0/energy_uj", socket))?);
    Ok(reading)
}

fn parse_rapl_energy(rapl_energy_file: String) -> Result<u64, io::Error> {
    Ok(fs::read_to_string(rapl_energy_file)?.trim().parse().unwrap())
}
