use std::fs;
use std::io;

use procfs::CpuTime;
use procfs::process::Stat;
use serde_json;

use crate::sample::{Sample, RaplReading};

fn serialize_task(task: &Stat) -> serde_json::Value {
    serde_json::json!([task.pid, task.processor, task.cutime, task.cstime])
}

fn serialize_cpu(cpu: i32, stat: &CpuTime) -> serde_json::Value {
    serde_json::json!([
        cpu,
        stat.user,
        stat.nice,
        stat.system,
        stat.idle,
        stat.iowait,
        stat.irq,
        stat.softirq,
        stat.steal,
        stat.guest,
        stat.guest_nice
    ])
}

fn serialize_rapl_reading(reading: &RaplReading) -> serde_json::Value {
    serde_json::json!([reading.socket, reading.dram as u64, reading.pkg as u64])
}

fn serialize_sample(sample: &Sample) -> Option<serde_json::Value> {
    match sample {
        Sample::Task(sample) => {
            if let Some(tasks) = &sample.tasks {
                // return Some(serde_json::json!({
                //     sample.timestamp.to_string(): serde_json::value::Value::Array(tasks
                //         .iter()
                //         .map(serialize_task)
                //         .collect::<Vec<serde_json::Value>>())
                // }))

                let mut values = serde_json::map::Map::new();
                for task in tasks.into_iter() {
                    values.insert(sample.timestamp.to_string(), serialize_task(task));
                }
                println!("{:?}", serde_json::to_string_pretty(&values));
                return None
                // return Some(serde_json::value::Value::Object(values))
            }
        }
        Sample::Cpu(sample) => {
            if let Some(cpus) = &sample.cpus {
                return Some(serde_json::json!({
                    sample.timestamp.to_string(): serde_json::value::Value::Array(cpus
                        .iter()
                        .enumerate()
                        .map(|(cpu, stat)| -> serde_json::Value { serialize_cpu(cpu as i32, stat) })
                        .collect::<Vec<serde_json::Value>>())
                }))
            }
        }
        Sample::Rapl(sample) => {
            if let Some(readings) = &sample.readings {
                return Some(serde_json::json!({
                    sample.timestamp.to_string(): serde_json::value::Value::Array(readings
                        .iter()
                        .map(serialize_rapl_reading)
                        .collect::<Vec<serde_json::Value>>())
                }))
            }
        }
        // _ => return None
    }
    None
}

pub fn write_data(samples: Vec<Sample>, output_path: String) -> Result<(), io::Error> {
    let data = serde_json::json!({
        "task": samples
                    .iter()
                    .filter(|s| match s {
                        Sample::Task(_) => true,
                        _ => false
                    })
                    .map(serialize_sample)
                    .filter_map(|s| s)
                    .collect::<Vec<serde_json::Value>>(),
        "cpu": samples
                    .iter()
                    .filter(|s| match s {
                        Sample::Cpu(_) => true,
                        _ => false
                    })
                    .map(serialize_sample)
                    .filter_map(|s| s)
                    .collect::<Vec<serde_json::Value>>(),
        "rapl": samples
                    .iter()
                    .filter(|s| match s {
                        Sample::Rapl(_) => true,
                        _ => false
                    })
                    .map(serialize_sample)
                    .filter_map(|s| s)
                    .collect::<Vec<serde_json::Value>>()
    });
    fs::write(output_path, serde_json::to_string_pretty(&data)?)
}
