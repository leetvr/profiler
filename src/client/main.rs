use benchmarks::ProfileSummary;
use chrono::Utc;
use log::{debug, info, trace};
use redis::AsyncCommands;
use redis_ts::AsyncTsCommands;
use std::{
    collections::HashMap,
    process::{Command, Stdio},
    time::Duration,
};
const RUN_DURATION: Duration = Duration::from_secs(5);
const PROFILER_COMMAND: &str = r#"ovrgpuprofiler -r"3,4,5,6,7,8,16,17,40,42,43,44,45""#;
const TARGET_FRAME_TIME: f32 = 13.0; // TODO: does this include TW + guardian and friends?
use anyhow::Result;

#[derive(Debug, Clone)]
struct Sample {
    value: f32,
    timestamp: u64, // milliseconds since unix UTC
}
type Metrics = HashMap<String, Vec<Sample>>;

#[tokio::main]
async fn main() -> Result<()> {
    pretty_env_logger::init();
    let description = get_description();
    let start_time = chrono::Utc::now();
    enable_ovr_metrics();
    let pid = launch();
    wait_for_focused(pid);
    let output = get_profile_data(pid);
    kill();

    disable_ovr_metrics();
    save_profile_metrics(output, description, start_time).await
}

fn get_description() -> String {
    // get the description as an argument, else prompt for it
    if let Some(description) = std::env::args().nth(1) {
        description
    } else {
        println!("[HOTHAM_PROFILER] What did you change?");
        let mut buffer = String::new();
        std::io::stdin().read_line(&mut buffer).unwrap();
        buffer.trim_end().to_string()
    }
}

fn enable_ovr_metrics() {
    adb("am broadcast -n com.oculus.ovrmonitormetricsservice/.SettingsBroadcastReceiver -a com.oculus.ovrmonitormetricsservice.ENABLE_CSV");
}

fn disable_ovr_metrics() {
    adb("am broadcast -n com.oculus.ovrmonitormetricsservice/.SettingsBroadcastReceiver -a com.oculus.ovrmonitormetricsservice.DISABLE_CSV");
}

async fn save_profile_metrics(
    mut metrics: Metrics,
    description: String,
    start_time: chrono::DateTime<Utc>,
) -> Result<()> {
    info!("Saving profiler output..");
    trace!("Profile metrics: {metrics:#?}");

    let client = redis::Client::open("redis://127.0.0.1/")?;
    let mut con = client.get_async_connection().await?;

    // Add the run to the list to get its index
    let mut averaged_metrics = get_averages(&metrics);
    let result = *averaged_metrics.get("Total Frame Time").unwrap() <= TARGET_FRAME_TIME;

    let runs_key = "profile_runs";
    let list_size: usize = con.llen(runs_key).await?;
    let id = list_size;

    // Save profile summary
    let summary = ProfileSummary {
        id,
        result,
        description,
        timestamp: start_time.timestamp_millis() as _,
    };
    let _ = con.lpush(runs_key, serde_json::to_string(&summary)?).await?;

    let summary_key = format!("profile_runs:{id}:summary");
    con.hset(&summary_key, "description", summary.description)
        .await?;
    con.hset(&summary_key, "timestamp", summary.timestamp)
        .await?;
    con.hset(&summary_key, "result", summary.result).await?;

    // Save the average metrics as a hash
    let metrics_key = format!("profile_runs:{id}:metrics");
    for (key, value) in averaged_metrics.drain() {
        con.hset(&metrics_key, key, value).await?;
    }

    // Save each metric
    let options = redis_ts::TsOptions::default().labels(vec![("profile_run", &id.to_string())]);
    for (name, samples) in metrics.drain() {
        for sample in samples {
            let _ = con
                .ts_add_create(&name, sample.timestamp, sample.value, options.clone())
                .await
                .map_err(|e| anyhow::format_err!("Error adding {name} {sample:?} - {e:?}"))?;
        }
    }

    Ok(())
}

fn get_profile_data(pid: usize) -> Metrics {
    let mut metrics = Default::default();
    get_gpu_metrics(&mut metrics);
    get_ovr_metrics(&mut metrics, pid);

    metrics
}

fn get_gpu_metrics(metrics: &mut Metrics) {
    info!("Profiling for {} seconds..", RUN_DURATION.as_secs());
    let start_time = chrono::Utc::now();
    let mut process = Command::new("adb")
        .args(["shell", "-tt", PROFILER_COMMAND])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    std::thread::sleep(RUN_DURATION);
    process.kill().unwrap();
    let output = String::from_utf8(process.wait_with_output().unwrap().stdout).unwrap();

    // GPU metrics get printed every second, so we can just cheat here and add 1000ms each time
    parse_gpu_metrics(&output)
        .drain()
        .for_each(|(key, mut values)| {
            let mut timestamp = (start_time.timestamp_millis() as u64) - 1000; // since we add 1000ms for each sample, start with 1000ms less
            let samples = values
                .drain(..)
                .map(|value| Sample {
                    value,
                    timestamp: {
                        timestamp += 1000;
                        timestamp
                    },
                })
                .collect();

            metrics.insert(key, samples);
        });
}

fn get_ovr_metrics(metrics: &mut Metrics, pid: usize) {
    let ovr_output = adb(format!("logcat -s VrApi --pid={pid} -t 5 -v epoch"));
    parse_ovr_metrics(&ovr_output, metrics)
}

fn wait_for_focused(pid: usize) -> () {
    // Wait for the app to start
    loop {
        let output = adb(format!(
            "logcat -d --pid {pid} | grep 'State is now FOCUSED'"
        ));
        if !output.is_empty() {
            break;
        }

        debug!("App not focused yet, waiting for 100");
        std::thread::sleep(Duration::from_millis(100));
    }

    info!("App is now focussed! Profiling..");
}

/// launch the thing
/// Returns the pid of the process
fn launch() -> usize {
    info!("LAUNCH");

    // Kill
    kill();

    // Start
    run()
}

fn kill() {
    info!("Killing app..");
    adb("am force-stop rust.the_station");
}

/// Runs a shell command
fn adb<T: AsRef<str>>(command: T) -> String {
    String::from_utf8(
        Command::new("adb")
            .arg("shell")
            .args(command.as_ref().split_whitespace())
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim_end()
    .to_string()
}

fn run() -> usize {
    info!("Running application");

    let build_output = String::from_utf8(
        Command::new("cargo")
            .args(["apk", "build", "--release"])
            .current_dir("../")
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    debug!("Build output: {}", build_output);

    let install_output = String::from_utf8(
        Command::new("adb")
            .args(["install", "target/release/apk/the_station.apk"])
            .current_dir("../")
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();

    debug!("Install output: {}", install_output);

    let run_output = adb("am start rust.the_station/android.app.NativeActivity");
    debug!("Run output: {}", run_output);

    info!("Application started!");

    get_pid()
}

fn get_pid() -> usize {
    loop {
        let output = adb("pidof rust.the_station");
        if output.is_empty() {
            debug!("App not started yet, waiting for 100");
            std::thread::sleep(Duration::from_millis(100));
            continue;
        }
        debug!("Received pid output {output:?}");

        return output.parse().unwrap();
    }
}

fn parse_gpu_metrics(output_string: &str) -> HashMap<String, Vec<f32>> {
    let mut metrics: HashMap<String, Vec<f32>> = Default::default();

    // First, split the string by double newlines to get a sample
    for sample in output_string.split("\n\n") {
        // Next, split the sample by \r\r\n (??) to get metrics
        for metric in sample.trim().split("\r\r\n") {
            let metric = metric.trim();
            if metric.is_empty() {
                continue;
            }
            // The name and the value are split by a : separator
            let mut split = metric.trim().split(":");
            let name = split.next().unwrap().trim().to_string();
            let value = split.next().unwrap().trim().parse::<f32>().unwrap();
            metrics.entry(name).or_default().push(value);
        }
    }

    metrics
}

fn get_averages(metrics: &Metrics) -> HashMap<String, f32> {
    let mut averaged_metrics = HashMap::new();
    for (name, samples) in metrics {
        let averaged = samples
            .iter()
            .fold(0., |accumulator, sample| accumulator + sample.value)
            / samples.len() as f32;
        averaged_metrics.insert(name.to_string(), averaged);
    }
    averaged_metrics
}

/// Parses lines that look something like:
///
/// ```bash
///          1668142546.002  3409  1858 I VrApi   : FPS=90/90,Prd=29ms,Tear=0,Early=0,Stale=0,VSnc=0,Lat=-1,Fov=0,CPU4/GPU=2/3,2419/490MHz,OC=FF,TA=0/70/0,SP=N/F/N,Mem=2092MHz,Free=3756MB,PLS=0,Temp=25.7C/0.0C,TW=1.90ms,App=4.27ms,GD=0.00ms,CPU&GPU=6.86ms,LCnt=2(DR0,LM0),GPU%=0.59,CPU%=0.19(W0.24),DSF=1.00,CFL=14.46/19.67
/// ```
fn parse_ovr_metrics(output_string: &str, metrics: &mut Metrics) {
    let mut ovr_metrics: Metrics = Default::default();

    // First, split the string by newlines to get a sample
    for sample in output_string.split("\n") {
        if !sample.contains("VrApi") {
            continue;
        }
        // The interesting stuff starts on the other side of the colon
        let mut split = sample.split(": ");
        let start = split.next().unwrap().trim();

        // Take 14 characters and turn that into a timestamp
        let (timestamp, _) = start.split_at(14);
        let timestamp = timestamp.replace(".", "").parse::<u64>().unwrap();

        let data = split.next().unwrap();

        // Next, split the sample by "," to get metrics
        for metric in data.trim().split(",") {
            let metric = metric.trim();
            if metric.is_empty() {
                continue;
            }
            // The name and the value are split by a = separator
            let mut split = metric.trim().split("=");
            let name = split.next().unwrap().trim().to_string();
            let value = if name == "App" || name == "CPU&GPU" {
                let value = split.next().unwrap().trim();
                value.split("ms").next().unwrap().parse().unwrap()
            } else {
                continue;
            };
            ovr_metrics
                .entry(name)
                .or_default()
                .push(Sample { value, timestamp });
        }
    }

    let total_frame_time = ovr_metrics.remove("CPU&GPU").unwrap();
    let gpu_time = ovr_metrics.remove("App").unwrap();
    let cpu_time = total_frame_time
        .iter()
        .zip(&gpu_time)
        .map(|(total, gpu)| Sample {
            value: total.value - gpu.value,
            timestamp: total.timestamp,
        })
        .collect();

    metrics.insert("Total Frame Time".into(), total_frame_time);
    metrics.insert("GPU Time".into(), gpu_time);
    metrics.insert("CPU Time".into(), cpu_time);
}

#[cfg(test)]
mod tests {
    use crate::{parse_gpu_metrics, parse_ovr_metrics};

    #[test]
    pub fn test_parse_gpu_metrics() {
        let test_string = r#"Clocks / Second                            :   427468384.000
GPU % Bus Busy                             :          12.054
% Vertex Fetch Stall                       :           9.646

Clocks / Second                            :   478906784.000
GPU % Bus Busy                             :           9.189
% Vertex Fetch Stall                       :          10.290

Clocks / Second                            :   395378048.000
GPU % Bus Busy                             :          10.143
% Vertex Fetch Stall                       :          10.761

Clocks / Second                            :   395940032.000
GPU % Bus Busy                             :          10.171
% Vertex Fetch Stall                       :          10.965

Clocks / Second                            :   396080448.000
GPU % Bus Busy                             :          10.150
% Vertex Fetch Stall                       :          10.868"#
            .to_string();

        let metrics = parse_gpu_metrics(&test_string);
        assert!(metrics.contains_key("Clocks / Second"));
    }

    //     #[test]
    //     fn test_parse_ovr_metrics() {
    //         let mut metrics = Default::default();
    //         let test_string = r#"--------- beginning of main
    // 11-10 12:51:47.536 19594 19943 I VrApi   : FPS=43/72,Prd=42ms,Tear=1,Early=0,Stale=34,VSnc=0,Lat=-1,Fov=0,CPU4/GPU=4/4,2419/490MHz,OC=FF,TA=0/0/0,SP=N/N/N,Mem=2092MHz,Free=3079MB,PLS=0,Temp=35.7C/0.0C,TW=3.01ms,App=8.09ms,GD=0.00ms,CPU&GPU=23.57ms,LCnt=4(DR13,LM0),GPU%=0.86,CPU%=0.05(W0.10),DSF=1.00,CFL=19.66/21.37
    // --------- beginning of crash
    // 11-10 12:51:48.587 19594 19943 I VrApi   : FPS=43/72,Prd=42ms,Tear=1,Early=0,Stale=34,VSnc=0,Lat=-1,Fov=0,CPU4/GPU=4/4,2419/490MHz,OC=FF,TA=0/0/0,SP=N/N/N,Mem=2092MHz,Free=3044MB,PLS=0,Temp=35.7C/0.0C,TW=3.01ms,App=8.09ms,GD=0.00ms,CPU&GPU=23.57ms,LCnt=4(DR13,LM0),GPU%=0.86,CPU%=0.05(W0.10),DSF=1.00,CFL=19.66/21.37 "#;
    //         let ovr_metrics = parse_ovr_metrics(&mut metrics, test_string);
    //         assert_eq!(ovr_metrics.total_frame_time, 23.57);
    //         assert_eq!(ovr_metrics.gpu_time, 8.09);
    //         assert_eq!(
    //             ovr_metrics.cpu_time,
    //             ovr_metrics.total_frame_time - ovr_metrics.gpu_time
    //         );
    //     }
}
