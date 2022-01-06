use anyhow::Context as _;

mod pb {
    tonic::include_proto!("hako");
}
pub use pb::deployment_server::DeploymentServer;

#[derive(Debug)]
pub struct DeploymentService {
    config: DeploymentServiceConfig,
    s3_client: aws_sdk_s3::Client,
}

#[derive(Debug)]
pub struct DeploymentServiceConfig {
    pub s3_bucket: String,
    pub s3_prefix: String,
}

impl DeploymentService {
    pub fn new(config: DeploymentServiceConfig, shared_config: &aws_config::Config) -> Self {
        Self {
            config,
            s3_client: aws_sdk_s3::Client::new(shared_config),
        }
    }
}

#[tonic::async_trait]
impl pb::deployment_server::Deployment for DeploymentService {
    async fn deploy_oneshot(
        &self,
        request: tonic::Request<pb::DeployOneshotRequest>,
    ) -> Result<tonic::Response<pb::DeployOneshotResponse>, tonic::Status> {
        if request.get_ref().app_id.is_empty() {
            return Err(tonic::Status::invalid_argument("app_id must be present"));
        }
        wrap(self.deploy_oneshot_impl(request).await).await
    }
}

async fn wrap<T>(
    result: anyhow::Result<Result<T, tonic::Status>>,
) -> Result<tonic::Response<T>, tonic::Status> {
    match result {
        Ok(Ok(r)) => Ok(tonic::Response::new(r)),
        Ok(Err(s)) => Err(s),
        Err(e) => {
            tracing::error!(?e);
            Err(tonic::Status::internal(e.to_string()))
        }
    }
}

impl DeploymentService {
    async fn deploy_oneshot_impl(
        &self,
        request: tonic::Request<pb::DeployOneshotRequest>,
    ) -> anyhow::Result<Result<pb::DeployOneshotResponse, tonic::Status>> {
        let current_key = format!("{}/current", self.config.s3_prefix);
        let resp = self
            .s3_client
            .get_object()
            .bucket(&self.config.s3_bucket)
            .key(&current_key)
            .send()
            .await
            .with_context(|| {
                format!(
                    "failed to get current object s3://{}/{}",
                    self.config.s3_bucket, current_key
                )
            })?;
        let body = resp
            .body
            .collect()
            .await
            .with_context(|| {
                format!(
                    "failed to read contents of current object s3://{}/{}",
                    self.config.s3_bucket, current_key
                )
            })?
            .into_bytes();
        let revision = String::from_utf8(body.to_vec()).with_context(|| {
            format!(
                "failed to read contents of current object s3://{}/{} as UTF-8",
                self.config.s3_bucket, current_key
            )
        })?;
        let revision = revision.trim_end();

        let key = format!(
            "{}/{}/{}.json",
            self.config.s3_prefix,
            revision,
            request.get_ref().app_id
        );
        let result = self
            .s3_client
            .get_object()
            .bucket(&self.config.s3_bucket)
            .key(&key)
            .send()
            .await;
        if matches!(
            result,
            Err(aws_sdk_s3::SdkError::ServiceError {
                err: aws_sdk_s3::error::GetObjectError {
                    kind: aws_sdk_s3::error::GetObjectErrorKind::NoSuchKey(_),
                    ..
                },
                ..
            })
        ) {
            return Ok(Err(tonic::Status::not_found(format!(
                "{} is not registered",
                request.get_ref().app_id
            ))));
        }
        let resp = result.with_context(|| {
            format!(
                "failed to get definition s3://{}/{}",
                self.config.s3_bucket, key
            )
        })?;
        let body = resp
            .body
            .collect()
            .await
            .with_context(|| {
                format!(
                    "failed to read contents of definition s3://{}/{}",
                    self.config.s3_bucket, key
                )
            })?
            .into_bytes();

        let definition: crate::definition::Definition = serde_json::from_slice(&body)
            .with_context(|| {
                format!(
                    "failed to deserialize definition s3://{}/{}",
                    self.config.s3_bucket, key
                )
            })?;
        tracing::info!(?definition);

        // TODO: Perform ecs:RegisterTaskDefinition

        Ok(Ok(pb::DeployOneshotResponse {
            task_definition_arn: "arn:aws:todo".to_owned(),
        }))
    }
}
