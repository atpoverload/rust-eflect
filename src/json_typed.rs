use std::fs;
use std::io;

use procfs::CpuTime;
use procfs::process::Stat;
use serde_json;

use crate::sample::Sample;

fn serialize_task(task: &Stat) -> serde_json::Value {
    serde_json::json!({
        "thread_id": task.pid,
        "cpu": task.processor,
        "user": task.cutime,
        "system": task.cstime
    })
}

fn serialize_cpu(cpu: i32, stat: &CpuTime) -> serde_json::Value {
    serde_json::json!({
        "cpu": cpu,
        "user": stat.user,
        "nice": stat.nice,
        "system": stat.system,
        "idle": stat.idle,
        "iowait": stat.iowait,
        "irq": stat.irq,
        "softirq": stat.softirq,
        "steal": stat.steal,
        "guest": stat.guest,
        "guest_nice": stat.guest_nice
    })
}

fn serialize_sample(sample: &Sample) -> Option<serde_json::Value> {
    match sample {
        Sample::Task(sample) => {
            if let Some(tasks) = &sample.tasks {
                return Some(serde_json::json!({
                    "type": "task",
                    "timestamp": sample.timestamp as u64,
                    "tasks": serde_json::value::Value::Array(tasks
                        .iter()
                        .map(serialize_task)
                        .collect::<Vec<serde_json::Value>>())
                }))
            }
        }
        Sample::Cpu(sample) => {
            if let Some(cpus) = &sample.cpus {
                return Some(serde_json::json!({
                    "type": "cpu",
                    "timestamp": sample.timestamp as u64,
                    "cpus": serde_json::value::Value::Array(cpus
                        .iter()
                        .enumerate()
                        .map(|(cpu, stat)| -> serde_json::Value { serialize_cpu(cpu as i32, stat) })
                        .collect::<Vec<serde_json::Value>>())
                }))
            }
        }
    }
    None
}

pub fn write_data(samples: Vec<Sample>, output_path: String) -> Result<(), io::Error> {
    let data = serde_json::value::Value::Array(samples
        .iter()
        .map(serialize_sample)
        .filter(Option::is_some)
        .map(Option::unwrap)
        .collect::<Vec<serde_json::Value>>());
    fs::write(output_path, serde_json::to_string_pretty(&data)?)
}
