use log::info;
use redmine_service::reports_server::Reports;
use reqwest::Url;
use time::{macros::format_description, Date};
use tonic::{Request, Response, Status};
#[cfg(feature = "trace")]
use tracing::instrument;

use self::redmine_service::{report_response::PerUserReport, ReportRequest, ReportResponse};

#[derive(Debug)]
pub struct ReportService {
    redmine: crate::model::Redmine,
}

pub mod redmine_service {
    tonic::include_proto!("redmine_api");
}

impl ReportService {
    pub fn new(site: Url, api_key: String) -> Self {
        Self {
            redmine: crate::model::Redmine::new(site, api_key),
        }
    }
}

#[tonic::async_trait]
impl Reports for ReportService {
    #[cfg_attr(feature = "trace", instrument)]
    async fn generate_report(
        &self,
        request: Request<ReportRequest>,
    ) -> Result<Response<ReportResponse>, Status> {
        use crate::view::time_entries::aggregate_report;

        info!("Got a request from {:?}", request.remote_addr());

        let request = request.into_inner();
        let format = format_description!("[year]-[month]-[day]");
        let from = Date::parse(&request.generate_from_ts, &format)
            .map_err(|_| Status::invalid_argument("generate_from_ts"))?;
        let to = Date::parse(&request.generate_to_ts, &format)
            .map_err(|_| Status::invalid_argument("generate_to_ts"))?;

        let reports = aggregate_report(&self.redmine, &request.user_id, from, to).await?;

        let reply = ReportResponse {
            reports: reports
                .into_iter()
                .map(|report| PerUserReport {
                    user_id: report.user_id,
                    report: report.report,
                })
                .collect(),
        };

        Ok(Response::new(reply))
    }
}
