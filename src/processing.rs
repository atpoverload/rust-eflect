use crate::sample::Sample;

use std::collections::HashMap;
use std::ops::Sub;
use std::time::Instant;

// maybe these need to be rates during the sub
#[derive(Debug, Clone, Copy)]
pub enum Record {
    Task {
        start: Instant,
        end: Instant,
        id: u32,
        cpu: u32,
        user: u128,
        system: u128
    },
    Cpu {
        start: Instant,
        end: Instant,
        cpu: u32,
        user: u128,
        nice: u128,
        system: u128,
        idle: u128,
        iowait: u128,
        irq: u128,
        softirq: u128,
        steal: u128,
        guest: u128,
        guest_nice: u128
    },
    Energy {
        start: Instant,
        end: Instant,
        socket: u32,
        core: f32,
        dram: f32,
        gpu: f32,
        package: f32,
    },
}

impl Record {
    fn rate(&self) -> f32 {
        match self {
            Record::Task {
                start,
                end,
                user,
                system,
                ..
            } => (user + system) as f32 / (*end - *start).as_millis() as f32,
            Record::Cpu {
                start,
                end,
                user,
                nice,
                system,
                irq,
                softirq,
                steal,
                guest,
                guest_nice,
                ..
            } => (user + nice + system + irq + softirq + steal + guest + guest_nice) as f32 / (*end - *start).as_millis() as f32,
            Record::Energy {
                start,
                end,
                core,
                dram,
                gpu,
                package,
                ..
            } => (core + dram + gpu + package) / (*end - *start).as_millis() as f32,
            // _ => 0.0,
        }
    }
}

impl Sub<&Sample> for &Sample {
    type Output = Option<Record>;

    fn sub(self, other: &Sample) -> Option<Record> {
        if self.get_timestamp() > other.get_timestamp() {
            return None
        }

        match (self, other) {
            (Sample::Task(sample1), Sample::Task(sample2))
                => {
                    if sample1.id != sample2.id {
                        return None
                    }
                    Some(Record::Task {
                        start: sample1.timestamp,
                        end: sample2.timestamp,
                        id: sample1.id,
                        cpu: sample1.cpu,
                        user: sample2.user - sample1.user,
                        system: sample2.system - sample1.system
                    })
            },
            (Sample::Cpu(sample1), Sample::Cpu(sample2))
                => {
                    if sample1.cpu != sample2.cpu {
                        return None
                    }
                    Some(Record::Cpu {
                        start: sample1.timestamp,
                        end: sample2.timestamp,
                        cpu: sample1.cpu,
                        user: sample2.user - sample1.user,
                        nice: sample2.nice - sample1.nice,
                        system: sample2.system - sample1.system,
                        idle: sample2.idle - sample1.idle,
                        iowait: sample2.iowait - sample1.iowait,
                        irq: sample2.irq - sample1.irq,
                        softirq: sample2.softirq - sample1.softirq,
                        steal: sample2.steal - sample1.steal,
                        guest: sample2.guest - sample1.guest,
                        guest_nice: sample2.guest_nice - sample1.guest_nice
                    })
            },
            (Sample::Energy(sample1), Sample::Energy(sample2))
                => {
                    if sample1.socket != sample2.socket {
                        return None
                    }
                    Some(Record::Energy {
                        start: sample1.timestamp,
                        end: sample2.timestamp,
                        socket: sample1.socket,
                        core: sample2.core - sample1.core,
                        dram: sample2.dram - sample1.dram,
                        gpu: sample2.gpu - sample1.gpu,
                        package: sample2.package - sample1.package,
                    })
            },
            (_, _) => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct TaskActivity {
    start: Instant,
    end: Instant,
    id: u32,
    cpu: u32,
    activity: f32,
}

fn group_samples(samples: Vec<Sample>) -> HashMap<u32, HashMap<u32, Vec<Sample>>> {
    let mut grouped_samples: HashMap<u32, HashMap<u32, Vec<Sample>>> = HashMap::new();
    let mut add_sample = |sample: Sample, key| grouped_samples
        .entry(sample.key())
        .or_insert_with(HashMap::new)
        .entry(key)
        .or_insert_with(Vec::new)
        .push(sample);
    samples.iter().for_each(|sample| {
        match sample {
            Sample::Task(task) => add_sample(*sample, task.id),
            Sample::Cpu(cpu) => add_sample(*sample, cpu.cpu),
            Sample::Energy(energy) => add_sample(*sample, energy.socket),
        }
    });
    grouped_samples
}

fn forward_difference(samples: HashMap<u32, HashMap<u32, Vec<Sample>>>) -> HashMap<u128, Vec<Option<Record>>> {
    let mut grouped_records: HashMap<u128, Vec<Option<Record>>> = HashMap::new();
    samples.values().flatten().for_each(|(_, samples_)| {
        let mut samples_ = samples_.clone();
        samples_.sort();
        for n in 1..samples_.len() {
            let timestamp = samples_.get(n - 1).unwrap().get_timestamp().elapsed().as_millis() / 50;
            let record = samples_.get(n - 1).unwrap() - samples_.get(n).unwrap();
            grouped_records.entry(timestamp).or_insert_with(Vec::new).push(record);
        }
    });
    grouped_records
}

fn extract_cpu_data(records: Vec<Option<Record>>) -> HashMap<u32, f32> {
    let mut cpus: HashMap<u32, f32> = HashMap::new();
    for record in records {
        match record {
            // Some(Record::Cpu {cpu, ..}) => cpus.insert(cpu, record.unwrap().rate()),
            Some(Record::Cpu {cpu, ..}) => *cpus.entry(cpu / 12).or_insert(0.0) += record.unwrap().rate(),
            _ => continue,
        };
    };
    cpus
}

fn account_tasks(records: Vec<Option<Record>>, cpus: HashMap<u32, f32>) -> Vec<TaskActivity> {
    let mut tasks: Vec<TaskActivity> = Vec::new();
    if cpus.is_empty() {
        return tasks
    }
    for record in records {
        match record {
            Some(Record::Task {start, end, id, cpu, ..}) =>
                match (record.unwrap().rate(), cpus.get(&(cpu / 12))) {
                    (task_rate, Some(cpu_rate)) if task_rate > 0.0 && *cpu_rate > 0.0 =>
                        tasks.push(TaskActivity {
                            start : start,
                            end : end,
                            id : id,
                            cpu : cpu,
                            activity: task_rate / cpu_rate
                        }),
                _ => continue,
                },
            _ => continue,
        };
    };
    tasks
}

pub fn process(samples: Vec<Sample>) {
    // group samples by type
    let grouped_samples = group_samples(samples);

    // take the forward difference of the data
    let grouped_records = forward_difference(grouped_samples);

    let data: Vec<TaskActivity> = grouped_records.values().map(|records|
        account_tasks(records.to_vec(), extract_cpu_data(records.to_vec()))
    ).flatten().collect();
    println!("{:?}", data);

    // group samples by type; i want an enum map
    // let mut tasks: HashMap<u32, Vec<Sample>> = HashMap::new();
    // let mut cpus: HashMap<u32, Vec<Sample>> = HashMap::new();
    // let mut energies: HashMap<u32, Vec<Sample>> = HashMap::new();
    //     match sample {
    //         Sample::Task(task) => data.entry(sample).or_insert_with(Vec::new).push(*sample),
    //         // Sample::Cpu(cpu) => cpus.entry(cpu.cpu).or_insert_with(Vec::new).push(*sample),
    //         // Sample::Energy(energy) => energies.entry(energy.socket).or_insert_with(Vec::new).push(*sample),
    //         _ => {},
    //     };
    // });
    //
    // // let mut records: Vec<Option<Record>> = Vec::new();
    // let mut records: HashMap<Instant, Vec<Option<Record>>> = HashMap::new();
    // tasks.iter().for_each(|(_, samples)| {
    //     let mut samples = samples.clone();
    //     samples.sort();
    //     for n in 1..samples.len() {
    //         records.entry(samples.get(n - 1).unwrap().get_timestamp()).or_insert_with(Vec::new).push(samples.get(n - 1).unwrap().sub(*samples.get(n).unwrap()));
    //     }
    // });
    // cpus.iter().for_each(|(_, samples)| {
    //     let mut samples = samples.clone();
    //     samples.sort();
    //     for n in 1..samples.len() {
    //         records.entry(samples.get(n - 1).unwrap().get_timestamp()).or_insert_with(Vec::new).push(samples.get(n - 1).unwrap().sub(*samples.get(n).unwrap()));
    //     }
    // });
    // records
}
