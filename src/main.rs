use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use clap::App;
use ctrlc;
use procfs::process::Process;
use protobuf::Message;
use protobuf::ProtobufError;

use eflect::Eflect;
use eflect::json::write_data;

fn main() {
    let matches = App::new("eflect")
                      .arg_from_usage("<pid> 'The id of the process to monitor'")
                      .arg_from_usage("-p, --period=[PERIOD] 'The sampling period in milliseconds'")
                      .arg_from_usage("-o, --output=[OUTPUT] 'Location to write the output data'")
                      .get_matches();

    if let Some(pid) = matches.value_of("pid") {
        if let Ok(pid) = pid.parse() {
            println!("EFLECT: monitoring process {:?}", pid);
            // build the collector
            let mut eflector = match matches.value_of("period") {
                Some(period) => Eflect::for_process_with_period(pid, period.parse().unwrap()),
                None => Eflect::for_process(pid)
            };

            let running = Arc::new(AtomicBool::new(true));
            let r = running.clone();

            ctrlc::set_handler(move || {
                r.store(false, Ordering::SeqCst);
            }).expect("Error setting Ctrl-C handler");

            // profile the running process
            eflector.start();

            if let Ok(process) = Process::new(pid) {
                while process.is_alive() & running.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(1));
                }
            }

            eflector.stop();

            // write the data
            let samples = eflector.read();
            let output = match matches.value_of("output") {
                Some(filename) => filename,
                None => "eflect-data.pb"
            };

            let mut data_set = eflect_stack_trace::StackTraceDataSet::new();
            self.traces_to_proto().into_iter().for_each(|trace| data_set.samples.push(trace));
            self.frames_to_proto().into_iter().for_each(|frame| data_set.frames.push(frame));
            // self.add_traces(&data_set);
            // self.add_frames(&data_set);
            match data_set.write_to_writer(w) {
                // this is the only possible failure since i filled the proto
                Err(ProtobufError::IoError(err)) => bail!(err),
                _ => Ok(())
            }

            if let Err(error) = write_data(samples, output.to_string()) {
                println!("EFLECT: failed to write data: {:?}", error);
            } else {
                println!("EFLECT: wrote data for process {:?}", pid);
            }
        };
    } else {
        println!("no pid was provided!");
    }
}
