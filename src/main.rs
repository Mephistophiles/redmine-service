use std::env;

use anyhow::{Context, Result};
use controller::redmine_service::reports_server::ReportsServer;
use log::info;
use tonic::transport::Server;

mod controller;
mod model;
mod view;

fn init_log() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp(None)
        .parse_default_env()
        .init();

    log::trace!("Log initialization: This is trace message");
    log::debug!("Log initialization: This is debug message");
    log::info!("Log initialization: This is info message");
    log::warn!("Log initialization: This is warn message");
    log::error!("Log initialization: This is error message");
}

#[cfg(feature = "trace")]
fn init_tracing() {
    use tracing_subscriber::{fmt, layer::SubscriberExt};

    // Create a tracing layer with the configured tracer
    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name("redmine-service")
        .install_simple()
        .unwrap();
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    tracing::subscriber::set_global_default(
        fmt::Subscriber::builder()
            // subscriber configuration
            .with_max_level(tracing::Level::INFO)
            .finish()
            // add additional writers
            .with(telemetry)
            .with(fmt::Layer::default()),
    )
    .expect("Unable to set global tracing subscriber");
    log::debug!("Tracing initialized.");
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    init_log();

    #[cfg(feature = "trace")]
    init_tracing();

    let addr = env::var("GRPC_ADDR")
        .with_context(|| "env GRPC_ADDR was not found, please export it".to_string())?
        .parse()
        .unwrap();

    info!("Listening on {}", addr);

    Server::builder()
        .add_service(ReportsServer::new(controller::ReportService::new(
            env::var("REDMINE_URL")
                .with_context(|| "env REDMINE_URL was not found, please export it".to_string())?
                .parse()?,
            env::var("REDMINE_API_KEY").with_context(|| {
                "env REDMINE_API_KEY was not found, please export it".to_string()
            })?,
        )))
        .serve(addr)
        .await
        .with_context(|| "GRPC Server was not started".to_string())?;

    Ok(())
}
