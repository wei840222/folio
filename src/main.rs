mod config;
mod files;
mod uploads;
mod workflow;

#[macro_use]
extern crate rocket;
extern crate log;
extern crate pretty_env_logger;

use anyhow::Result;
use figment::providers::{Env, Format, Serialized, Toml};
use rocket::fs::FileServer;
use std::str::FromStr;
use std::sync::Arc;
use temporalio_common::telemetry::TelemetryOptionsBuilder;
use temporalio_common::worker::{WorkerConfigBuilder, WorkerTaskTypes, WorkerVersioningStrategy};
use temporalio_sdk::{Worker, sdk_client_options};
use temporalio_sdk_core::{CoreRuntime, RuntimeOptionsBuilder, Url, init_worker};

#[get("/")]
fn health() -> &'static str {
    "OK"
}

#[launch]
async fn rocket() -> _ {
    pretty_env_logger::init();
    let figment = rocket::Config::figment()
        .merge(Serialized::defaults(config::Folio::default()))
        .merge(Toml::file("Folio.toml").nested())
        .merge(Env::prefixed("FOLIO_").global());

    let temporal_figment = rocket::Config::figment()
        .merge(Serialized::defaults(config::Temporal::default()))
        .merge(Toml::file("Folio.toml").nested())
        .merge(Env::prefixed("TEMPORAL_").global());

    let mut config: config::Folio = figment.extract().unwrap();
    config.temporal = temporal_figment.extract().unwrap();
    log::info!("Using config: {:?}", config);

    // Initialize Temporal
    let temporal_client = init_temporal(&config)
        .await
        .expect("Failed to connect to Temporal");

    rocket::custom(figment)
        .mount("/health", routes![health])
        .mount("/uploads", routes![uploads::upload_file])
        .mount(
            "/files",
            routes![files::create_file, files::upsert_file, files::delete_file],
        )
        .mount(
            "/files",
            FileServer::from(config.uploads_path.to_string()).rank(5),
        )
        .mount("/", FileServer::from(config.web_path.to_string()))
        .manage(config)
        .manage(Some(temporal_client))
}

async fn init_temporal(
    config: &config::Folio,
) -> Result<Arc<temporalio_client::RetryClient<temporalio_sdk_core::Client>>> {
    let url = Url::from_str(&format!("http://{}", config.temporal.address))?;

    let client_options = sdk_client_options(url.clone())
        .identity(hostname::get().unwrap().to_string_lossy().to_string())
        .build()?;

    let client = client_options
        .connect(&config.temporal.namespace, None)
        .await?;
    log::info!("Connected to Temporal Server {}", url);
    let client_arc = Arc::new(client);

    // Setup Worker
    let telemetry_options = TelemetryOptionsBuilder::default().build()?;
    let runtime_options = RuntimeOptionsBuilder::default()
        .telemetry_options(telemetry_options)
        .build()?;
    let runtime = CoreRuntime::new_assume_tokio(runtime_options)?;

    // Setup Worker Config
    let worker_config = WorkerConfigBuilder::default()
        .namespace(&config.temporal.namespace)
        .task_queue(&config.temporal.task_queue)
        .task_types(WorkerTaskTypes::all())
        .versioning_strategy(WorkerVersioningStrategy::None {
            build_id: "v1".to_owned(),
        })
        .build()?;

    let client_for_worker = client_arc.clone();
    let task_queue = config.temporal.task_queue.clone();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            // Initialize Worker inside the thread
            let core_worker = init_worker(&runtime, worker_config, client_for_worker)
                .expect("Failed to init worker");
            let mut worker = Worker::new_from_core(Arc::new(core_worker), task_queue);

            worker.register_wf(
                "file_expiration_workflow",
                workflow::file_expiration_workflow,
            );
            worker.register_activity("delete_file_activity", workflow::delete_file_activity);

            log::info!("⚙️ Starting Temporal Worker...");
            worker.run().await.expect("Failed to run worker");
        });
    });

    Ok(client_arc)
}
