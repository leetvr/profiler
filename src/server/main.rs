use anyhow::Result;
use benchmarks::{MetricSummary, ProfileRun, ProfileSummary};
use futures::{stream::FuturesUnordered, TryStreamExt};
use redis::AsyncCommands;
use redis_ts::{AsyncTsCommands, TsRange};
use std::{collections::HashMap, convert::Infallible};
use warp::Filter;

#[tokio::main]
pub async fn main() {
    let cors = warp::cors().allow_any_origin();

    let profile_list = warp::path("profiles").and_then(get_profiles).with(&cors);

    let profiles = warp::path("profiles")
        .and(warp::path::param())
        .and_then(get_profile)
        .with(&cors);

    let route = profiles.or(profile_list);

    warp::serve(route).run(([0, 0, 0, 0], 8888)).await;
}

async fn get_profiles() -> Result<impl warp::Reply, Infallible> {
    let response = match fetch_profiles().await {
        Ok(profiles) => warp::reply::json(&profiles),
        Err(e) => panic!("GOT AN ERROR - handle this at some point {e:?}"),
    };

    Ok(response)
}

async fn fetch_profiles() -> Result<Vec<ProfileSummary>> {
    let client = redis::Client::open("redis://127.0.0.1/")?;
    let mut con = client.get_async_connection().await?;
    let run_list: Vec<String> = con.lrange("profile_runs", 0, -1).await?;
    Ok(run_list
        .iter()
        .map(|s| serde_json::from_str(s).unwrap())
        .collect())
}

async fn get_profile(id: usize) -> Result<impl warp::Reply, Infallible> {
    let response = match get_profile_run(id).await {
        Ok(profile_summary) => warp::reply::json(&profile_summary),
        Err(e) => panic!("GOT AN ERROR - handle this at some point {e:?}"),
    };

    Ok(response)
}

async fn get_profile_run(id: usize) -> Result<ProfileRun> {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_async_connection().await?;
    let profile_summary: HashMap<String, String> =
        con.hgetall(format!("profile_runs:{id}:summary")).await?;
    let description = profile_summary
        .get("description")
        .ok_or_else(|| anyhow::format_err!("no description found?"))?
        .clone();
    let timestamp = profile_summary
        .get("timestamp")
        .ok_or_else(|| anyhow::format_err!("no timestamp found?"))?
        .clone();
    let result = profile_summary
        .get("result")
        .ok_or_else(|| anyhow::format_err!("no result found?"))?
        .clone();

    let metrics = get_metrics(con, id).await?;

    Ok(ProfileRun {
        description,
        timestamp: timestamp.parse().unwrap(),
        metrics,
        result: result.parse().unwrap(),
    })
}

async fn get_metrics(
    mut con: redis::aio::Connection,
    id: usize,
) -> Result<Vec<MetricSummary>, anyhow::Error> {
    let mut profile_metrics: HashMap<String, f32> =
        con.hgetall(format!("profile_runs:{id}:metrics")).await?;
    let metric_names = profile_metrics.keys().cloned().collect::<Vec<_>>();
    let average_metrics = get_average_metrics(metric_names).await?;
    let last_profile_id = id - 1;
    let last_profile_metrics: HashMap<String, f32> = con
        .hgetall(format!("profile_runs:{last_profile_id}:metrics"))
        .await?;
    let metrics = profile_metrics
        .drain()
        .map(|(name, value)| MetricSummary {
            last_value: last_profile_metrics.get(&name).copied().unwrap_or_default(), // might not exist
            average_value: average_metrics.get(&name).copied().unwrap(), // guaranteed to exist
            value,
            name,
        })
        .collect();
    Ok(metrics)
}

async fn get_average_metrics(mut metric_names: Vec<String>) -> Result<HashMap<String, f32>> {
    metric_names
        .drain(..)
        .map(get_average_value_for_metric)
        .collect::<FuturesUnordered<_>>()
        .try_collect()
        .await
}

async fn get_average_value_for_metric(name: String) -> Result<(String, f32)> {
    let client = redis::Client::open("redis://127.0.0.1/").unwrap();
    let mut con = client.get_async_connection().await?;

    let average: TsRange<u64, f32> = con
        .ts_revrange(
            &name,
            "-",
            "+",
            None as Option<String>, // *shakes fist*
            Some(redis_ts::TsAggregationType::Avg(100000000000)),
        )
        .await?;

    Ok((name, average.values[0].1))
}
