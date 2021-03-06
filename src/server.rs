// a simple server implementation for eflect on linux. only one process can be monitored at a time.
// /proc/stat, /proc/pid/task, and /sys/class/powercap are each sampled from their own thread.
// TODO(timur): switch to an executor so we can divide up the readings?
mod protos {
    tonic::include_proto!("eflect.protos.sample");
}

use std::fs;
use std::io;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, Receiver, channel};
use std::vec::Vec;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use env_logger;
use log;
use log::{LevelFilter, error, info, warn};

use procfs::{CpuTime, KernelStats, ProcError};
use procfs::process::{Process, Stat};

use tonic::{Request, Response, Status};
use tonic::transport::Server;

use protos::{CpuSample, CpuStat, TaskSample, TaskStat, RaplReading, RaplSample, Sample};
use protos::{DataSet, StartRequest, StartResponse, StopRequest, StopResponse, ReadRequest, ReadResponse, sample::Data};
use protos::sampler_server::{Sampler, SamplerServer};

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

// TODO(timur): make real error handling
struct SamplingError {
    message: String
}

// code to sample /proc/stat
fn sample_cpus() -> Result<Sample, SamplingError> {
    match read_cpus() {
        Ok(stats) => {
            let mut sample = CpuSample::default();
            sample.timestamp = now_ms();
            stats.into_iter().for_each(|stat| sample.stat.push(stat));
            let mut s = Sample::default();
            s.data = Some(Data::Cpu(sample));
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
    let mut stat_proto = CpuStat::default();
    stat_proto.cpu = cpu;
    stat_proto.user = Some(stat.user as u32);
    stat_proto.nice = Some(stat.nice as u32);
    stat_proto.system = Some(stat.system as u32);
    stat_proto.idle = Some(stat.idle as u32);
    if let Some(jiffies) = stat.iowait {
        stat_proto.iowait = Some(jiffies as u32);
    };
    if let Some(jiffies) = stat.irq {
        stat_proto.irq = Some(jiffies as u32);
    };
    if let Some(jiffies) = stat.softirq {
        stat_proto.softirq = Some(jiffies as u32);
    };
    if let Some(jiffies) = stat.steal {
        stat_proto.steal = Some(jiffies as u32);
    };
    if let Some(jiffies) = stat.guest {
        stat_proto.guest = Some(jiffies as u32);
    };
    if let Some(jiffies) = stat.guest_nice {
        stat_proto.guest_nice = Some(jiffies as u32);
    };
    stat_proto
}

// code to sample /proc/[pid]/task/[tid]/stat
fn sample_tasks(pid: i32) -> Result<Sample, SamplingError> {
    if let Ok(tasks) = read_tasks(pid) {
        let mut sample = TaskSample::default();
        sample.timestamp = now_ms();
        tasks.into_iter().for_each(|s| sample.stat.push(s));
        let mut s = Sample::default();
        s.data = Some(Data::Task(sample));
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
    let mut stat_proto = TaskStat::default();
    stat_proto.task_id = stat.pid as u32;
    if let Some(cpu) = stat.processor {
        stat_proto.cpu = cpu as u32;
        stat_proto.user = Some(stat.utime as u32);
        stat_proto.system = Some(stat.stime as u32);
    };
    stat_proto
}

// code to sample /sys/class/powercap
fn sample_rapl() -> Result<Sample, SamplingError> {
    if let Ok(reading) = read_rapl() {
        let mut sample = RaplSample::default();
        sample.timestamp = now_ms();
        reading.into_iter().for_each(|reading| sample.reading.push(reading));
        let mut s = Sample::default();
        s.data = Some(Data::Rapl(sample));
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

// TODO(timur): implement handling for K components
fn read_socket(socket: u32) -> Result<RaplReading, io::Error> {
    let mut reading = RaplReading::default();
    reading.socket = socket;
    reading.package = Some(parse_rapl_energy(format!("/sys/class/powercap/intel-rapl:{}/energy_uj", socket))?);
    reading.dram = Some(parse_rapl_energy(format!("/sys/class/powercap/intel-rapl:{}:0/energy_uj", socket))?);
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

    use crate::protos::sample::Data;

    #[test]
    // make sure sample_cpus returns the right data type
    fn jiffies_smoke_test() {
        let start = now_ms();
        if let Ok(sample) = sample_cpus() {
            if let Some(Data::Cpu(sample)) = sample.data {
                assert!(sample.timestamp <= now_ms());
                assert!(sample.timestamp >= start);
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
            if let Some(Data::Task(sample)) = sample.data {
                assert!(sample.timestamp <= now_ms());
                assert!(sample.timestamp >= start);
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
            if let Some(Data::Rapl(sample)) = sample.data {
                assert!(sample.timestamp <= now_ms());
                assert!(sample.timestamp >= start);
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

// sampler implementation
struct SamplerImpl {
    period: Duration,
    is_running: Arc<AtomicBool>,
    sender: Arc<Mutex<Sender<Sample>>>,
    receiver: Arc<Mutex<Receiver<Sample>>>,
}

impl SamplerImpl {
    fn start_sampling_from<F>(&self, mut source: F)
        where F: FnMut() -> Result<Sample, SamplingError> + Send + Sync + Clone + 'static {
        let is_running = self.is_running.clone();
        let sender = self.sender.clone();
        let period = self.period;

        // TODO(timur): look into switching to scheduled_executor
        thread::spawn(move || {
                while is_running.load(Ordering::Relaxed) {
                    let start = Instant::now();
                    // TODO(timur): make the logging configurable?
                    match source() {
                        Ok(sample) => sender.lock().unwrap().send(sample).unwrap(),
                        // TODO(timur): we need to be able to abandon this if the source is broken
                        Err(error) => error!("{}", error.message),
                    };
                    thread::sleep(period - (Instant::now() - start));
                }
        });
    }
}

impl Default for SamplerImpl {
    fn default() -> SamplerImpl {
        let (tx, rx) = channel::<Sample>();
        SamplerImpl {
            period: Duration::from_millis(100),
            is_running: Arc::new(AtomicBool::new(false)),
            sender: Arc::new(Mutex::new(tx)),
            receiver: Arc::new(Mutex::new(rx)),
        }
    }
}

#[tonic::async_trait]
impl Sampler for SamplerImpl {
    async fn start(
        &self,
        request: Request<StartRequest>,
    ) -> Result<Response<StartResponse>, Status> {
        if !self.is_running.load(Ordering::Relaxed) {
            // check the pid first in case we need to abandon?
            let pid: i32 = request.into_inner().pid.unwrap() as i32;
            info!("start requested for pid={}", pid);

            self.is_running.store(true, Ordering::Relaxed);

            self.start_sampling_from(sample_cpus);
            self.start_sampling_from(sample_rapl);
            self.start_sampling_from(move || sample_tasks(pid));
        } else {
            warn!("ignoring start request while collecting");
        }

        Ok(Response::new(StartResponse {}))
    }

    async fn stop(
        &self,
        _: Request<StopRequest>,
    ) -> Result<Response<StopResponse>, Status> {
        info!("stop requested");
        self.is_running.store(false, Ordering::Relaxed);
        Ok(Response::new(StopResponse {}))
    }

    async fn read(
        &self,
        _: Request<ReadRequest>,
    ) -> Result<Response<ReadResponse>, Status> {
        info!("read requested");
        let response = if !self.is_running.load(Ordering::Relaxed) {
            let mut data = DataSet::default();
            let receiver = self.receiver.lock().unwrap();
            while let Ok(sample) = receiver.try_recv() {
                match sample.data {
                    Some(Data::Cpu(sample)) => data.cpu.push(sample),
                    Some(Data::Task(sample)) => data.task.push(sample),
                    Some(Data::Rapl(sample)) => data.rapl.push(sample),
                    _ => log::warn!("no sample found!")
                }
            }
            let empty = data.cpu.is_empty() && data.rapl.is_empty() && data.task.is_empty();
            ReadResponse {data: if empty { None } else { Some(data) }}
        } else {
            warn!("ignoring read request while collecting");
            ReadResponse {data: None}
        };
        Ok(Response::new(response))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::builder().filter_level(LevelFilter::Info).init();
    let addr = "[::1]:50051".parse().unwrap();
    let sampler = SamplerImpl::default();

    info!("eflect listening on {}", addr);

    Server::builder()
        .add_service(SamplerServer::new(sampler))
        .serve(addr)
        .await?;

    Ok(())
}
