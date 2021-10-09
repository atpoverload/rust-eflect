use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use clap::App;
use procfs::process::Process;
use protobuf::Message;
use protobuf::ProtobufError;

use eflect::Sampler;

fn main() {
    let matches = App::new("eflect")
                      .arg_from_usage("--pid=<pid> 'The id of the process to monitor'")
                      .arg_from_usage("-p, --period=[PERIOD] 'The sampling period in milliseconds'")
                      .arg_from_usage("-o, --output=[OUTPUT] 'Location to write the output data'")
                      .get_matches();

    if let Some(pid) = matches.value_of("pid") {
        if let Ok(pid) = pid.parse() {
            println!("EFLECT: monitoring process {:?}", pid);
            // build the collector
            let mut sampler = match matches.value_of("period") {
                Some(period) => Sampler::for_process_with_period(pid, period.parse().unwrap()),
                None => Sampler::for_process(pid)
            };

            let running = Arc::new(AtomicBool::new(true));
            let r = running.clone();

            ctrlc::set_handler(move || {
                r.store(false, Ordering::SeqCst);
            }).expect("Error setting Ctrl-C handler");

            // profile the running process
            sampler.start();

            if let Ok(process) = Process::new(pid) {
                while process.is_alive() & running.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(1));
                }
            }

            sampler.stop();

            // write the data
            let data_set = sampler.read();
            let mut out_file = std::fs::File::create(&matches.value_of("period").unwrap_or("eflect-data.pb")).unwrap();
            match data_set.write_to_writer(&mut out_file) {
                // this is the only possible failure since i filled the proto
                Err(ProtobufError::IoError(error)) => println!("EFLECT: failed to write data: {:?}", error),
                _ => println!("EFLECT: wrote data for process {:?} at {:?}", pid, out_file)
            };
        };
    } else {
        println!("no pid was provided!");
    }
}
