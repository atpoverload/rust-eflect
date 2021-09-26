use eflect::Eflect;
use eflect::processing;

use std::thread;
use std::time::Duration;

fn fibonacci(n: u32) -> u32 {
    match n {
        0 => 1,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

fn fake_workload() {
    println!("sleep time!");
    let mut handles: Vec<thread::JoinHandle<()>> = Vec::new();
    for _ in 0..11 {
        handles.push(thread::spawn(|| {
            thread::sleep(Duration::from_secs(1));
            fibonacci(38);
        }));
    }

    for handle in handles {
        handle.join().expect("couldn't join!");
    }
    println!("done sleeping!");
}

fn main() {
    let mut eflector = Eflect::new();
    // let mut eflector = Eflect::with_period_ms(500);
    eflector.start();

    fake_workload();

    eflector.stop();
    let samples = eflector.read();
    processing::process(samples)
    // println!("{:?}", processing::process(samples));
    // println!("{:?}", samples);
}
