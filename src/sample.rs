// Structures and methods for sampled data.
use std::fs;
use std::io;
use std::time::SystemTime;

use procfs::{CpuTime, KernelStats, ProcError};
use procfs::process::{Process, Stat};

use crate::protos::jiffies::{CpuSample, CpuStat, TaskSample, TaskStat};
use crate::protos::rapl::{RaplReading, RaplSample};
use crate::protos::sample::{Sample};

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
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
            sample.set_timestamp(now_ms());
            stats.into_iter().for_each(|stat| sample.stat.push(stat));
            let mut s = Sample::new();
            s.set_cpu(sample);
            Ok(s)
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
        sample.set_timestamp(now_ms());
        tasks.into_iter().for_each(|s| sample.stat.push(s));
        let mut s = Sample::new();
        s.set_task(sample);
        Ok(s)
    } else {
        Err(SamplingError{message: "there was an error with proc".to_string()})
    }
}

fn read_tasks(pid: i32) -> Result<Vec<TaskStat>, ProcError> {
    Ok(Process::new(pid)?.tasks()?
        .flatten()
        .filter_map(|stat| stat.stat().ok())
        .map(|stat| task_stat_to_proto(stat))
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
        sample.set_timestamp(now_ms());
        reading.into_iter().for_each(|reading| sample.reading.push(reading));
        let mut s = Sample::new();
        s.set_rapl(sample);
        Ok(s)
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

#[cfg(test)]
mod tests {
    use super::*;

    use procfs::CpuInfo;
    use procfs::process::Process;

    use crate::protos::sample::{Sample_oneof_data};

    #[test]
    // make sure sample_cpus returns the right data type
    fn jiffies_smoke_test() {
        let start = now_ms();
        if let Ok(sample) = sample_cpus() {
            if let Some(Sample_oneof_data::cpu(sample)) = sample.data {
                assert!(sample.get_timestamp() <= now_ms());
                assert!(sample.get_timestamp() >= start);
                // TODO(timur): make sure that there's no duplicate cpus
                assert_eq!(sample.stat.len(), CpuInfo::new().unwrap().num_cores());
            } else {
                panic!("sampling cpus failed; data other than CpuSample returned");
            };
        } else {
            panic!("sampling cpus failed; /proc/stat couldn't be read");
        };

        let start = now_ms();
        let me = Process::myself().unwrap();
        if let Ok(sample) = sample_tasks(me.pid) {
            if let Some(Sample_oneof_data::task(sample)) = sample.data {
                assert!(sample.get_timestamp() <= now_ms());
                assert!(sample.get_timestamp() >= start);
                // TODO(timur): no good way to check if the threads in there are valid
                assert_eq!(sample.stat.len(), me.tasks().unwrap().count());
            } else {
                panic!("sampling tasks failed; data other than TaskSample returned");
            };
        } else {
            panic!("sampling tasks failed; /proc/[pid]/task couldn't be read");
        };
    }

    #[test]
    // make sure we have rapl (/sys/class/powercap) and that we read all the values
    fn rapl_smoke_test() {
        let start = now_ms();
        if let Ok(sample) = sample_rapl() {
            if let Some(Sample_oneof_data::rapl(sample)) = sample.data {
                assert!(sample.get_timestamp() <= now_ms());
                assert!(sample.get_timestamp() >= start);
                // TODO(timur): have to check for components
                // assert_eq!(sample.stat.len(), CpuInfo::new().unwrap().num_cores());
            } else {
                panic!("sampling rapl failed; data other than RaplSample returned");
            };
        } else {
            panic!("sampling rapl failed; /sys/class/powercap couldn't be read");
        };
    }
}
