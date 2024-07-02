use chrono::prelude::*;
use chrono_tz::{Asia::Tokyo, Tz};
use cron::Schedule;
use std::{
    future::Future,
    marker::{Send, Sync},
    str::FromStr,
    time::Duration,
};
use tokio::{signal::ctrl_c, spawn, task::JoinHandle, time::sleep};
use tokio_util::sync::CancellationToken;

fn ctrl_c_handler(token: CancellationToken) -> JoinHandle<()> {
    spawn(async move {
        ctrl_c().await.unwrap();
        println!("received ctrl-c");
        token.cancel();
    })
}

fn make_looper<F1, F2>(
    token: CancellationToken,
    expression: &'static str,
    // f: impl Fn(&DateTime<Utc>) -> F1 + Send + Sync + 'static,
    f: impl Fn(&DateTime<Tz>) -> F1 + Send + Sync + 'static,
    g: impl Fn() -> F2 + Send + Sync + 'static,
) -> JoinHandle<()>
where
    F1: Future<Output = ()> + Send,
    F2: Future<Output = ()> + Send,
{
    spawn(async move {
        let schedule = Schedule::from_str(expression).unwrap();
        // let mut next_tick = schedule.upcoming(Utc).next().unwrap();
        let mut next_tick = schedule.upcoming(Tokyo).next().unwrap();
        loop {
            if token.is_cancelled() {
                g().await;
                break;
            }

            // let now = Utc::now();
            let now = Utc::now().with_timezone(&Tokyo);
            if now >= next_tick {
                // 指定時刻になったら処理を実行
                f(&now).await;

                // 次回の実行時刻を計算
                // next_tick = schedule.upcoming(Utc).next().unwrap();
                next_tick = schedule.upcoming(Tokyo).next().unwrap();
            }

            // 次回の実行までの待機時間を計算
            sleep(Duration::from_secs(std::cmp::min(
                (next_tick - now).num_seconds() as u64,
                60,
            ))).await;
        }
    })
}

#[tokio::main]
async fn main() {
    let token = CancellationToken::new();
    let handles = vec![
        make_looper(
            token.clone(),
            "*/10 * * * * *",
            |&now: &_| async move {
                println!("定期処理1: {}", now);
                let resp = reqwest::get("https://httpbin.org/get")
                    .await
                    .unwrap()
                    .json::<serde_json::Value>()
                    .await
                    .unwrap();
                println!("{:#?}", resp);
            },
            || async move {
                println!("graceful stop looper 1");
            },
        )
    ];

    ctrl_c_handler(token);
    for handle in handles {
        handle.await.unwrap();
    }
}
